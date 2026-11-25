use zikaron::json::Value;

struct OneAsk(Box<dyn crate::rpc::Endpoint + Send>);

fn capped(method: &str) -> bool {
    method != crate::rpc::BROADCAST && method != "eth_getLogs" && method != "eth_getTransactionReceipt"
}

impl crate::rpc::Endpoint for OneAsk {
    fn call(&mut self, method: &str, params: &Value) -> Result<crate::wire::W, crate::rpc::Trouble> {
        if capped(method) {
            self.0.call_within(method, params, crate::send::ONE_ASK)
        } else {
            self.0.call(method, params)
        }
    }
    fn call_within(&mut self, method: &str, params: &Value, within: std::time::Duration) -> Result<crate::wire::W, crate::rpc::Trouble> {
        let within = if capped(method) { within.min(crate::send::ONE_ASK) } else { within };
        self.0.call_within(method, params, within)
    }
    fn waits_out_limits(&self, method: &str) -> bool {
        self.0.waits_out_limits(method)
    }
    fn asks_a_node(&self) -> bool {
        self.0.asks_a_node()
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, crate::rpc::Trouble>> {
        self.0.call_batch(asks)
    }
    fn another(&self) -> Option<Box<dyn crate::rpc::Endpoint + Send>> {
        Some(Box::new(OneAsk(self.0.another()?)))
    }
    fn name(&self) -> String {
        self.0.name()
    }
    fn place(&self) -> String {
        self.0.place()
    }
}

pub fn one_ask(ep: Box<dyn crate::rpc::Endpoint + Send>) -> Box<dyn crate::rpc::Endpoint + Send> {
    Box::new(OneAsk(ep))
}

type Passed = Vec<(String, crate::judge::Missing)>;

thread_local! {
    static READ: std::cell::RefCell<Option<Passed>> = const { std::cell::RefCell::new(None) };
}

pub struct OneRead(bool);

impl OneRead {
    #[must_use]
    pub fn open() -> OneRead {
        OneRead(READ.with(|r| {
            let mut r = r.borrow_mut();
            let fresh = r.is_none();
            if fresh {
                *r = Some(Vec::new());
            }
            fresh
        }))
    }
}

impl Drop for OneRead {
    fn drop(&mut self) {
        if self.0 {
            READ.with(|r| *r.borrow_mut() = None);
        }
    }
}

pub fn one_read<R>(read: impl FnOnce() -> R) -> R {
    let _read = OneRead::open();
    read()
}

pub fn passed_over(url: &str) -> Option<crate::judge::Missing> {
    let place = crate::endpoints::place_of(url);
    READ.with(|r| r.borrow().as_ref().and_then(|p| p.iter().find(|(k, _)| *k == place).map(|(_, m)| m.clone())))
}

pub fn pass_over(url: &str, m: &crate::judge::Missing) {
    let place = crate::endpoints::place_of(url);
    READ.with(|r| {
        if let Some(p) = r.borrow_mut().as_mut() {
            if !p.iter().any(|(k, _)| *k == place) {
                p.push((place, m.clone()));
            }
        }
    });
}

pub fn passes_over(method: &str, t: &crate::rpc::Trouble) -> bool {
    match t {
        crate::rpc::Trouble::Transport(crate::rpc::Broke::Unserved { .. } | crate::rpc::Broke::NoKey { .. } | crate::rpc::Broke::LogsBehind { .. }) => false,
        crate::rpc::Trouble::Transport(_) => true,
        crate::rpc::Trouble::Node(_) => !crate::patience::lacks_the_method(method, t) && !crate::patience::answers_the_question(t) && !crate::patience::refuses_the_question(t),
        crate::rpc::Trouble::NotServed(_) | crate::rpc::Trouble::Contradiction(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_receipt_asked_alone_is_not_held_to_one_ask() {
        assert!(!capped("eth_getTransactionReceipt"), "a receipt");
        assert!(!capped("eth_getLogs") && !capped(crate::rpc::BROADCAST), "logs and a broadcast, as before");
        for m in ["eth_getBalance", "eth_blockNumber", "eth_getTransactionByHash", "eth_getTransactionCount", "eth_call"] {
            assert!(capped(m), "{m}: one ask's time");
        }
    }
}
