use std::{cell::{Cell,RefCell},collections::BTreeMap,rc::Rc,sync::Arc};
use cove_platform::InputEvent;
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub struct SubscriptionToken(u64);
/// Subscriber registry for platform events.
///
/// Swift's `PlatformEventBridge` is a class, so every holder shares one
/// subscription table. Cloning here shares the table rather than copying it —
/// otherwise a clone would silently stop receiving events published through the
/// original.
#[derive(Clone,Default)]
pub struct PlatformEventBridge(Arc<BridgeStorage>);
#[derive(Default)]
struct BridgeStorage{subscribers:RefCell<BTreeMap<SubscriptionToken,Rc<dyn Fn(&InputEvent)>>>,next:Cell<u64>}
impl PlatformEventBridge{
    pub fn new()->Self{Self::default()}
    pub fn subscribe(&self,handler:impl Fn(&InputEvent)+'static)->SubscriptionToken{let next=self.0.next.get().checked_add(1).expect("event subscription IDs exhausted");self.0.next.set(next);let token=SubscriptionToken(next);self.0.subscribers.borrow_mut().insert(token,Rc::new(handler));token}
    pub fn unsubscribe(&self,token:SubscriptionToken){self.0.subscribers.borrow_mut().remove(&token);}
    pub fn publish(&self,event:&InputEvent){let subscribers=self.0.subscribers.borrow().iter().map(|(&token,handler)|(token,handler.clone())).collect::<Vec<_>>();for(token,handler)in subscribers{if self.0.subscribers.borrow().contains_key(&token){handler(event);}}}
}
