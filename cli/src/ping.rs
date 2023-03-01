use routing::{network::ipv4::addr::IpV4Addr, transport::icmp::IcmpApi};
use tracing::{info, warn};

#[derive(Debug, clap::Args)]
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
) -> (u16, u16, Option<std::time::Duration>) {
    let start = std::time::Instant::now();
    (
        id,
        seq,
        match tokio::time::timeout(
            std::time::Duration::from_secs_f32(timeout),
            icmp_api.echo_ip_v4(id, seq, ip),
        )
        .await
        {
            Ok(Some(_)) => Some(std::time::Instant::now() - start),
            Ok(None) => {
                warn!("Error sending or receiving packet");
                None
            }
            Err(_) => None,
        },
    )
}

pub async fn ping(
    Ping {
        ip,
        timeout_secs,
        number,
    }: Ping,
    icmp_api: &IcmpApi,
) {
    let mut join_set = tokio::task::JoinSet::new();
    let id = 0;
    let n = number.unwrap_or(5);
    for s in 0..n {
        let icmp_api = icmp_api.clone();
        join_set.spawn(async move { echo(ip, timeout_secs, id, s as u16, &icmp_api).await });
        tokio::time::sleep(std::time::Duration::from_secs_f32(0.5)).await;
    }
    let mut res = Vec::with_capacity(n);
    while let Some(data) = join_set.join_next().await {
        if let Ok(data) = data {
            res.push(data);
        }
    }
    res.sort_by_key(|(a, b, _)| (*a, *b));
    print_stats(&res);
}

fn print_stats(data: &[(u16, u16, Option<std::time::Duration>)]) {
    let sent = data.len();
    let received = data.iter().filter(|(_, _, o)| o.is_some()).count();
    let lost = sent - received;
    let percent_lost = (lost as f32 * 100.) / (sent as f32);

    let (min, max, sum): (Option<std::time::Duration>, Option<std::time::Duration>, _) = data
        .iter()
        .filter_map(|(_, _, o)| *o)
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
            .filter_map(|(_, _, o)| *o)
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
