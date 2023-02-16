pub trait Link {
    type Addr;
    type Packet;
    type RecvError;
    type SendError;

    fn get_addr(&self) -> Self::Addr;

    fn recv(&self) -> Result<Self::Packet, Self::RecvError>;

    fn try_recv(&self) -> Result<Option<Self::Packet>, Self::RecvError>;

    fn send(&self, packet: Self::Packet, dest: Self::Addr) -> Result<(), Self::SendError>;
}
