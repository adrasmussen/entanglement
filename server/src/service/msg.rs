// we will likely just have main.rs set everything up w/o a dedicated service manager
#[derive(Debug)]
pub enum Svc {
    _Status,
    _Add,
    _Start,
    _Stop,
    _GetSender,
    _Ping,
}
