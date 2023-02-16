pub trait Payload<const MAX_BYTES: usize> {
    fn as_bytes(&self) -> Vec<u8>;
}

impl<const N: usize> Payload<N> for [u8; N] {
    fn as_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }
}

impl<const N: usize> Payload<N> for &[u8] {
    fn as_bytes(&self) -> Vec<u8> {
        self[..(self.len().min(N))].to_vec()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tcp<P>
where
    P: Payload<1460>,
{
    octet: u32,
    payload: P,
}

impl<P> Payload<1480> for Tcp<P>
where
    P: Payload<1460>,
{
    fn as_bytes(&self) -> Vec<u8> {
        todo!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Udp<P>
where
    P: Payload<1472>,
{
    octet: u32,
    payload: P,
}

impl<P> Payload<1480> for Udp<P>
where
    P: Payload<1472>,
{
    fn as_bytes(&self) -> Vec<u8> {
        todo!()
    }
}
