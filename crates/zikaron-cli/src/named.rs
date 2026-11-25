use crate::codes::{Key, Reason};
use crate::out::{self, s, Answer, Said};
use zikaron_glue::door::Own;
use zikaron_send::{Code, Trouble};

pub fn of(own: Own, code: &str, detail: &str, network: bool, place: &str) -> Answer {
    match own {
        Own::IssueRefused => {
            let members = vec![(Key::Token, s(code)), (Key::Detail, s(detail))];
            if network || code == Code::NoEndpoint.as_str() {
                out::unanswered(Reason::IssueRefused, members)
            } else {
                out::denied(Reason::IssueRefused, members)
            }
        }
        Own::ReadOnly => out::denied(Reason::ReadOnly, vec![(Key::Token, s(code)), (Key::Detail, s(detail))]).telling(Said::ReadOnly.text().to_string()),
        Own::HandedOff => out::denied(Reason::HandedOff, vec![(Key::Handoff, s(detail))]).telling(Said::HandedOff.text().to_string()),
        Own::SendRefused => send_refused(code, detail, network, Said::SendRefused.text().to_string()),
        Own::ChainUnread => chain_unread(code, detail, place),
        Own::Tip => out::denied(if code == zikaron_glue::door::TIP_NOT_MOVED { Reason::NotMoved } else { Reason::TipForked }, vec![]),
        Own::Face => {
            let members = vec![(Key::Token, s(code)), (Key::Detail, s(detail))];
            if network {
                out::unanswered(Reason::FaceRefused, members).telling(Said::FaceRefused.text().to_string())
            } else {
                out::denied(Reason::FaceRefused, members).telling(Said::FaceRefused.text().to_string())
            }
        }
    }
}

pub fn send_refused(code: &str, detail: &str, network: bool, line: String) -> Answer {
    if code == Code::TooEarly.as_str() {
        return crate::verbs::too_early(detail);
    }
    let members = vec![(Key::Token, s(code)), (Key::Detail, s(detail))];
    let a = if network { out::unanswered(Reason::SendRefused, members) } else { out::denied(Reason::SendRefused, members) };
    a.telling(line)
}

fn chain_unread(code: &str, detail: &str, place: &str) -> Answer {
    if code == Code::NoChainId.as_str() || code == Code::NoRegistry.as_str() {
        let place = std::fs::canonicalize(place).map(|p| p.display().to_string()).unwrap_or_else(|_| place.to_string());
        return out::unanswered(Reason::Scan, vec![(Key::Token, s("E_NO_BASIS")), (Key::Detail, s(&place))]);
    }
    let members = vec![(Key::Token, s(code)), (Key::Detail, s(detail))];
    if code == Code::Disagree.as_str() {
        return out::unanswered(Reason::EndpointsDisagree, members);
    }
    out::unanswered(Reason::Unreachable, members)
}

pub fn chain_unread_of(t: &Trouble) -> bool {
    matches!(t.code, Code::NoChainId | Code::NoRegistry | Code::Disagree) || t.network()
}

pub fn credential(t: &zikaron_seal::Trouble) -> Option<Answer> {
    use zikaron_seal::Code as C;
    let members = vec![(Key::Token, s(&crate::sealed::word(t.code))), (Key::Detail, s(&t.tail))];
    Some(match t.code {
        C::CredentialExpired => out::denied(Reason::CredentialExpired, members).telling(Said::CredentialExpired.text().to_string()),
        C::CredentialMissing => out::denied(Reason::CredentialMissing, members).telling(Said::CredentialMissing.text().to_string()),
        C::CredentialShape => out::denied(Reason::CredentialShape, members).telling(Said::CredentialShape.text().to_string()),
        C::CredentialPassMissing => out::denied(Reason::CredentialPassMissing, members).telling(Said::CredentialPassMissing.text().to_string()),
        C::CredentialPassUnknown => out::denied(Reason::CredentialPassUnknown, members).telling(Said::CredentialPassUnknown.text().to_string()),
        _ => return None,
    })
}

pub fn of_trouble(own: Own, t: &Trouble, place: &str) -> Answer {
    of(own, t.code.as_str(), &zikaron_send::said(&t.why), t.network(), place)
}
