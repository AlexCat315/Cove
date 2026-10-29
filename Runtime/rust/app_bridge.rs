use std::{cell::{Cell,RefCell},collections::BTreeMap,rc::Rc};
use cove_platform::InputEvent;
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct SubscriptionToken(u64);
#[derive(Default)]
pub struct PlatformEventBridge{subscribers:RefCell<BTreeMap<SubscriptionToken,Rc<dyn Fn(&InputEvent)>>>,next:Cell<u64>}
impl PlatformEventBridge{
    pub fn new()->Self{Self::default()}
    pub fn subscribe(&self,handler:impl Fn(&InputEvent)+'static)->SubscriptionToken{let next=self.next.get().checked_add(1).expect("event subscription IDs exhausted");self.next.set(next);let token=SubscriptionToken(next);self.subscribers.borrow_mut().insert(token,Rc::new(handler));token}
    pub fn unsubscribe(&self,token:SubscriptionToken){self.subscribers.borrow_mut().remove(&token);}
    pub fn publish(&self,event:&InputEvent){let subscribers=self.subscribers.borrow().iter().map(|(&token,handler)|(token,handler.clone())).collect::<Vec<_>>();for(token,handler)in subscribers{if self.subscribers.borrow().contains_key(&token){handler(event);}}}
}
