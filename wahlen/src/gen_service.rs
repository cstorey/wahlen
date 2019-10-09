use failure::Fallible;

pub trait GenService<Req> {
    type Resp;
    fn call(&mut self, req: Req) -> Fallible<Self::Resp>;
}
