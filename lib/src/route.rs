// destination | gateway | mask | iface

use std::{fmt::Display, ops::BitAnd};

pub trait AddrMask<Addr>: BitAnd<Addr, Output = Addr> {
    type Specifity: Ord;
    fn specifity(&self) -> Self::Specifity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingEntry<Addr, AddrMask> {
    destination: Addr,
    gateway: Addr,
    mask: AddrMask,
}

impl<Addr, AddrMask> RoutingEntry<Addr, AddrMask> {
    pub const fn new(destination: Addr, gateway: Addr, mask: AddrMask) -> Self {
        Self {
            destination,
            gateway,
            mask,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingTable<Addr, Mask> {
    data: Vec<RoutingEntry<Addr, Mask>>,
}

impl<Addr, Mask> RoutingTable<Addr, Mask> {
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn add_route(&mut self, route: RoutingEntry<Addr, Mask>)
    where
        Mask: AddrMask<Addr>,
    {
        let i = self
            .data
            .binary_search_by_key(&route.mask.specifity(), |entry| entry.mask.specifity())
            .map_or_else(|i| i, |i| i);
        self.data.insert(i, route);
    }

    pub fn remove_route(&mut self, route: &RoutingEntry<Addr, Mask>)
    where
        Mask: AddrMask<Addr> + Eq,
        Addr: Eq,
    {
        if let Some(i) =
            self.data
                .iter()
                .enumerate()
                .find_map(|(i, entry)| if entry == route { Some(i) } else { None })
        {
            self.data.remove(i);
        }
    }

    pub fn get_route(&self, addr: Addr) -> Option<Addr>
    where
        Mask: AddrMask<Addr> + Clone,
        Addr: Clone + Eq,
    {
        self.data
            .iter()
            .find(
                |RoutingEntry {
                     destination: dest,
                     gateway: _,
                     mask,
                 }| (mask.clone() & dest.clone()) == (mask.clone() & addr.clone()),
            )
            .map(
                |RoutingEntry {
                     destination: _,
                     gateway,
                     mask: _,
                 }| gateway.clone(),
            )
    }
}

impl<Addr, AddrMask> RoutingTable<Addr, AddrMask>
where
    Addr: Display,
    AddrMask: Display,
{
    pub fn print(&self) -> prettytable::Table {
        let mut table = prettytable::table!(["destination", "mask", "gateway"]);
        for RoutingEntry {
            destination,
            gateway,
            mask,
        } in self.data.iter()
        {
            table.add_row(prettytable::row![destination, mask, gateway]);
        }
        table
    }
}

impl<Addr, AddrMask> Default for RoutingTable<Addr, AddrMask> {
    fn default() -> Self {
        Self::new()
    }
}
