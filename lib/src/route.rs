// destination | gateway | mask | iface

use std::{fmt::Display, ops::BitAnd};

pub trait AddrMask<Addr>: BitAnd<Addr, Output = Addr> {
    type Specifity: Ord;
    fn specifity(&self) -> Self::Specifity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingEntry<Addr, AddrMask, Iface> {
    destination: Addr,
    gateway: Addr,
    mask: AddrMask,
    iface: Iface,
}

impl<Addr, AddrMask, Iface> RoutingEntry<Addr, AddrMask, Iface> {
    pub const fn new(destination: Addr, gateway: Addr, mask: AddrMask, iface: Iface) -> Self {
        Self {
            destination,
            gateway,
            mask,
            iface,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingTable<Addr, Mask, Iface> {
    data: Vec<RoutingEntry<Addr, Mask, Iface>>,
}

impl<Addr, Mask, Iface> RoutingTable<Addr, Mask, Iface> {
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn add_route(&mut self, route: RoutingEntry<Addr, Mask, Iface>)
    where
        Mask: AddrMask<Addr>,
    {
        let i = self
            .data
            .binary_search_by_key(&route.mask.specifity(), |entry| entry.mask.specifity())
            .map_or_else(|i| i, |i| i);
        self.data.insert(i, route);
    }

    pub fn remove_route(&mut self, route: &RoutingEntry<Addr, Mask, Iface>)
    where
        Mask: AddrMask<Addr> + Eq,
        Addr: Eq,
        Iface: Eq,
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

    pub fn get_route(&self, addr: Addr) -> Option<(Addr, Iface)>
    where
        Mask: AddrMask<Addr> + Clone,
        Addr: Clone + Eq,
        Iface: Clone,
    {
        self.data
            .iter()
            .find(
                |RoutingEntry {
                     destination: dest,
                     gateway: _,
                     mask,
                     iface: _,
                 }| (mask.clone() & dest.clone()) == (mask.clone() & addr.clone()),
            )
            .map(
                |RoutingEntry {
                     destination: _,
                     gateway,
                     mask: _,
                     iface,
                 }| (gateway.clone(), iface.clone()),
            )
    }
}

impl<Addr, AddrMask, Iface> RoutingTable<Addr, AddrMask, Iface>
where
    Addr: Display,
    AddrMask: Display,
    Iface: Display,
{
    pub fn print(&self) -> prettytable::Table {
        let mut table = prettytable::table!(["destination", "mask", "gateway", "iface"]);
        for RoutingEntry {
            destination,
            gateway,
            mask,
            iface,
        } in self.data.iter()
        {
            table.add_row(prettytable::row![destination, mask, gateway, iface]);
        }
        table
    }
}

impl<Addr, AddrMask, Iface> Default for RoutingTable<Addr, AddrMask, Iface> {
    fn default() -> Self {
        Self::new()
    }
}
