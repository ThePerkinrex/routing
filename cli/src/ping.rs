use std::sync::Arc;

use routing::{network::ipv4::addr::IpV4Addr, transport::icmp::IcmpApi};
use tokio::{select, sync::RwLock};
use tracing::{info, warn};

use crate::{chassis::ChassisData, command::chassis::ParsedChassisCommandRead, ctrlc::CtrlC};

#[derive(Debug, clap::Parser)]
pub struct Ping {
    ip: IpV4Addr,
    #[arg(long, short, default_value_t = 5.)]
    timeout_secs: f32,
    #[arg(short)]
    number: Option<usize>,
}

async fn echo(
    ip: IpV4Addr,
    timeout: f32,
    id: u16,
    seq: u16,
    icmp_api: &IcmpApi,
) -> Option<((u16, u16, IpV4Addr, u8), std::time::Duration)> {
    let start = std::time::Instant::now();

    match tokio::time::timeout(
        std::time::Duration::from_secs_f32(timeout),
        icmp_api.echo_ip_v4(id, seq, ip),
    )
    .await
    {
        Ok(Some(data)) => Some((data, std::time::Instant::now() - start)),
        Ok(None) => {
            warn!("Error sending or receiving packet");
            None
        }
        Err(_) => None,
    }
}

pub async fn ping(
    Ping {
        ip,
        timeout_secs,
        number,
    }: Ping,
    icmp_api: &IcmpApi,
    ctrlc: &CtrlC,
) {
    let join_set = Arc::new(RwLock::new(tokio::task::JoinSet::new()));
    let id = 0;
    let n = number;
    let res = Arc::new(RwLock::new(Vec::new()));
    let mut f = {
        let res = res.clone();
        let join_set = join_set.clone();
        let icmp_api = icmp_api.clone();
        tokio::spawn(async move {
            let range = n.map_or_else::<Box<dyn Iterator<Item = usize> + Send>, _, _>(
                || Box::new(0..),
                |n| Box::new(0..n),
            );
            for s in range {
                let icmp_api = icmp_api.clone();
                res.write().await.push(None);
                join_set.write().await.spawn(async move {
                    let res = echo(ip, timeout_secs, id, s as u16, &icmp_api).await;
                    if let Some(((id, seq, addr, ttl), time)) = res.as_ref() {
                        info!(
                            "Received reply from {addr} icmp_seq={seq} icmp_id={id} ttl={ttl} time={time:?}"
                        )
                    }
                    res
                });
                tokio::time::sleep(std::time::Duration::from_secs_f32(0.5)).await;
            }
        })
    };
    {
        let handler = ctrlc.add_handler().await;
        // let stop = CtrlC::new().unwrap();
        select! {
            _ = &mut f => {
                info!("Sent all")
            }
            _ = handler.next() => {
                info!("Ctrl-C")
            }
        };
    }
    if !f.is_finished() {
        f.abort();
    }
    while let Some(data) = join_set.write().await.join_next().await {
        if let Ok(Some(((_, seq, _, _), d))) = data {
            res.write().await[seq as usize] = Some(d)
        }
    }
    print_stats(&res.read().await);
}

fn print_stats(data: &[Option<std::time::Duration>]) {
    let sent = data.len();
    let received = data.iter().filter(|o| o.is_some()).count();
    let lost = sent - received;
    let percent_lost = (lost as f32 * 100.) / (sent as f32);

    let (min, max, sum): (Option<std::time::Duration>, Option<std::time::Duration>, _) = data
        .iter()
        .filter_map(|o| *o)
        .fold((None, None, None), |(min, max, sum), d| {
            (
                Some(min.map_or(d, |min| min.min(d))),
                Some(max.map_or(d, |max| max.max(d))),
                Some(sum.map_or(d, |sum| sum + d)),
            )
        });
    let avg = sum.map(|x| x / received as u32);
    let std_dev = avg.map(|avg| {
        data.iter()
            .filter_map(|o| *o)
            .map(|d| d.as_secs_f64() - avg.as_secs_f64())
            .sum::<f64>()
            / f64::sqrt(received as f64)
    });

    info!("Ping stats:");
    info!("		Sent = {sent} Received = {received} Lost = {lost} ({percent_lost:.2}% loss)");
    if let (Some(min), Some(max), Some(avg), Some(std_dev)) = (min, max, avg, std_dev) {
        info!("Round trip:");
        info!(
            "		Min = {min:?} Max = {max:?} Avg = {avg:?} Std.Dev. = {}{:?}",
            if std_dev < 0. { "-" } else { "+" },
            std::time::Duration::from_secs_f64(std_dev.abs())
        );
    }
}

pub struct PingCommand;

#[async_trait::async_trait]
impl ParsedChassisCommandRead<Ping> for PingCommand {
    async fn run(
        &mut self,
        args: Ping,
        ctrlc: &CtrlC,
        _: String,
        ChassisData { icmp, .. }: &ChassisData,
    ) -> bool {
        ping(args, icmp, ctrlc).await;
        false
    }
}
