use crate::grantx::Draft;
use crate::vaultx::Held;

pub fn prefill(h: &Held) -> Draft {
    Draft { upstream: h.id.clone(), work: h.work.clone(), ..Default::default() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guide {
    NeedKey,
    NeedGenesis,
    Ready,
}

impl Guide {
    pub fn as_str(self) -> &'static str {
        match self {
            Guide::NeedKey => "need_key",
            Guide::NeedGenesis => "need_genesis",
            Guide::Ready => "ready",
        }
    }
}

pub fn guide(has_key: bool, rooted: bool) -> Guide {
    if !has_key {
        Guide::NeedKey
    } else if !rooted {
        Guide::NeedGenesis
    } else {
        Guide::Ready
    }
}
