#[derive(Clone,Copy,Debug,Default)]
pub struct FrameDeadline{pub next:Option<f64>,interval:Option<f64>}
impl FrameDeadline{
    pub fn update(&mut self,interval:Option<f64>,now:f64){let interval=interval.filter(|v|v.is_finite()&&*v>0.0);if interval==self.interval{return;}self.interval=interval;self.next=interval.map(|_|now);}
    pub fn consume_if_due(&mut self,now:f64)->bool{let(Some(interval),Some(next))=(self.interval,self.next)else{return false;};if now<next{return false;}self.next=Some(next+((now-next)/interval).floor().mul_add(interval,interval));true}
    pub fn remaining(&self,now:f64)->Option<f64>{self.next.map(|next|(next-now).max(0.0))}
}
