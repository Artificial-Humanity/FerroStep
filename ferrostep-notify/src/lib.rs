//! ferrostep-notify — a decision surface nobody looks at is a record that
//! waits forever.
//!
//! **The message is defined here; delivering it is somebody else's problem**
//! (ROADMAP B3). A [`Notification`] says which record needs a person, why,
//! how urgently, and how to get back to it. That much is ours and does not
//! vary. Everything past it is an adapter implementing [`Notifier`], because
//! delivery mechanisms are genuinely unalike — a URL you post to, service
//! credentials and a payload envelope, a device token and a key-signed
//! request, a local program — and an interface shaped around whichever got
//! written first would quietly exclude the rest. The target to design
//! against is the one nobody has thought of yet.
//!
//! [`Ntfy`] is the maintained default: self-hostable, no account needed, and
//! the worked example somebody copies to write the next adapter. It earns no
//! standing in the interface.
//!
//! Nothing here polls, schedules, or decides when work runs. A caller — a
//! harness step, an operator's cron, a store-side trigger — invokes
//! [`Notifier::notify`] when *it* decides something needs a person; FerroStep
//! only defines what the message says.

use std::fmt;

/// How urgently a person is needed. Three steps, because every transport in
/// sight can express at least three and a finer scale would be false
/// precision the caller has to invent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    /// Informational: someone should look when convenient.
    Low,
    /// A record is waiting on a person and nothing else moves it.
    Normal,
    /// Waiting, and the loop behind it is blocked or spending real money.
    High,
}

/// What FerroStep says when something needs a person.
///
/// Which record, why, how urgently, and how to get back to it — nothing
/// transport-shaped. An adapter maps these onto whatever its target speaks.
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    /// The workflow whose record waits (its `name` from the definition).
    pub workflow: String,
    /// The record that needs the person.
    pub record: String,
    /// The state it waits in.
    pub state: String,
    /// Why a person is needed, in words a person reads.
    pub reason: String,
    pub urgency: Urgency,
    /// How to get back to it: a URL when the deployment has one. Optional
    /// because the zero-install deployment has no web surface to link.
    pub link: Option<String>,
}

/// Why a notification could not be delivered.
#[derive(Debug, Clone, PartialEq)]
pub enum NotifyError {
    /// The transport answered, and the answer was no.
    Refused(String),
    /// The transport could not be reached at all.
    Transport(String),
}

impl fmt::Display for NotifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotifyError::Refused(detail) => write!(f, "the notifier refused: {detail}"),
            NotifyError::Transport(detail) => write!(f, "could not reach the notifier: {detail}"),
        }
    }
}

impl std::error::Error for NotifyError {}

/// Something that can put a [`Notification`] in front of a person.
pub trait Notifier {
    fn notify(&self, notification: &Notification) -> Result<(), NotifyError>;
}

/// The maintained default: [ntfy](https://ntfy.sh) — publish-by-POST,
/// self-hostable, Apache-2.0, no account needed.
pub struct Ntfy {
    server: String,
    topic: String,
    token: Option<String>,
    agent: ureq::Agent,
}

impl Ntfy {
    /// A notifier publishing to `topic` on `server` (e.g. a self-hosted
    /// instance, or the public `https://ntfy.sh`).
    pub fn new(server: &str, topic: &str) -> Self {
        Ntfy {
            server: server.trim_end_matches('/').to_string(),
            topic: topic.trim_matches('/').to_string(),
            token: None,
            agent: ureq::Agent::new_with_config(
                ureq::Agent::config_builder().http_status_as_error(false).build(),
            ),
        }
    }

    /// The same notifier, authenticating with an access token — for servers
    /// whose topics are not world-writable.
    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }
}

impl Notifier for Ntfy {
    fn notify(&self, n: &Notification) -> Result<(), NotifyError> {
        // ntfy's priority scale is 1–5; the three-step urgency maps into it
        // leaving room on both ends.
        let priority = match n.urgency {
            Urgency::Low => "2",
            Urgency::Normal => "3",
            Urgency::High => "5",
        };
        let title = format!("{}: record {} needs a person", n.workflow, n.record);
        let body = format!("{}\n(state: {})", n.reason, n.state);
        let mut request = self
            .agent
            .post(format!("{}/{}", self.server, self.topic))
            .header("X-Title", &title)
            .header("X-Priority", priority);
        if let Some(link) = &n.link {
            request = request.header("X-Click", link);
        }
        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let mut resp = request
            .send(&body[..])
            .map_err(|e| NotifyError::Transport(e.to_string()))?;
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            Ok(())
        } else {
            let detail = resp.body_mut().read_to_string().unwrap_or_default();
            Err(NotifyError::Refused(format!("{status}: {detail}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// Serve one request, capture it whole, answer `status`.
    fn capture_one(status: u16) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 65536];
            let mut request = String::new();
            loop {
                let n = stream.read(&mut buffer).unwrap();
                request.push_str(&String::from_utf8_lossy(&buffer[..n]));
                let Some(headers_end) = request.find("\r\n\r\n") else { continue };
                let content_length = request
                    .lines()
                    .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                    .unwrap_or(0);
                if request.len() >= headers_end + 4 + content_length {
                    break;
                }
            }
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
            );
            tx.send(request).unwrap();
        });
        (base, rx)
    }

    fn waiting_record() -> Notification {
        Notification {
            workflow: "review-loop".to_string(),
            record: "42".to_string(),
            state: "escalated".to_string(),
            reason: "agent_passes spent (3 of 3); a release needs a decision".to_string(),
            urgency: Urgency::High,
            link: Some("https://board.example/records/42".to_string()),
        }
    }

    #[test]
    fn the_message_reaches_the_wire_with_all_four_answers_on_it() {
        // Which record, why, how urgently, how to get back to it — each must
        // survive the mapping onto the transport.
        let (base, rx) = capture_one(200);
        Ntfy::new(&base, "loops").notify(&waiting_record()).unwrap();
        let request = rx.recv().unwrap();
        assert!(request.starts_with("POST /loops "), "publish is a POST to the topic:\n{request}");
        assert!(request.contains("record 42"), "which record");
        assert!(request.contains("agent_passes spent"), "why");
        assert!(request.contains("x-priority: 5") || request.contains("X-Priority: 5"), "how urgently:\n{request}");
        assert!(request.contains("https://board.example/records/42"), "how to get back");
        assert!(request.contains("(state: escalated)"));
    }

    #[test]
    fn urgency_maps_into_the_transport_scale_with_room_on_both_ends() {
        for (urgency, wire) in [(Urgency::Low, "2"), (Urgency::Normal, "3")] {
            let (base, rx) = capture_one(200);
            let n = Notification { urgency, link: None, ..waiting_record() };
            Ntfy::new(&base, "loops").notify(&n).unwrap();
            let request = rx.recv().unwrap().to_lowercase();
            assert!(request.contains(&format!("x-priority: {wire}")), "{urgency:?}:\n{request}");
            assert!(!request.contains("x-click"), "no link, no header");
        }
    }

    #[test]
    fn a_token_travels_as_a_bearer_and_a_refusal_is_an_error() {
        let (base, rx) = capture_one(403);
        let outcome = Ntfy::new(&base, "loops")
            .with_token("tk_secret")
            .notify(&waiting_record());
        let request = rx.recv().unwrap().to_lowercase();
        assert!(request.contains("authorization: bearer tk_secret"));
        match outcome {
            Err(NotifyError::Refused(detail)) => assert!(detail.starts_with("403"), "{detail}"),
            other => panic!("a 403 is a refusal, got {other:?}"),
        }
    }

    #[test]
    fn an_unreachable_server_is_a_transport_error_not_a_panic() {
        // A port nobody listens on.
        let unreachable = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", l.local_addr().unwrap())
        };
        let outcome = Ntfy::new(&unreachable, "loops").notify(&waiting_record());
        assert!(matches!(outcome, Err(NotifyError::Transport(_))), "{outcome:?}");
    }
}
