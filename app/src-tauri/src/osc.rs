//! OSC 送信。C# 版の `OscSender` / `VrcChatboxSender` に相当する。

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

/// OSC の引数。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OscArg {
    Str(String),
    Int(i32),
    Bool(bool),
}

/// NUL 終端し、4 バイト境界まで 0 埋めして追加する。
fn push_padded(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(bytes);
    let padded = (bytes.len() + 1).div_ceil(4) * 4;
    out.resize(out.len() + (padded - bytes.len()), 0);
}

/// OSC メッセージのバイト列を組み立てる。T/F は引数データを持たない。
pub fn encode_message(address: &str, args: &[OscArg]) -> Vec<u8> {
    let mut out = Vec::new();
    push_padded(&mut out, address.as_bytes());

    let mut tags = String::from(",");
    let mut data: Vec<u8> = Vec::new();
    for arg in args {
        match arg {
            OscArg::Str(s) => {
                tags.push('s');
                push_padded(&mut data, s.as_bytes());
            }
            OscArg::Bool(b) => tags.push(if *b { 'T' } else { 'F' }),
            OscArg::Int(i) => {
                tags.push('i');
                data.extend_from_slice(&i.to_be_bytes());
            }
        }
    }
    push_padded(&mut out, tags.as_bytes());
    out.extend_from_slice(&data);
    out
}

/// ホストを解決する。IP 文字列でもホスト名でもよく、IPv4 を優先する。
fn resolve(host: &str, port: i32) -> io::Result<SocketAddr> {
    if !(0..=65535).contains(&port) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("ポートが範囲外です: {port}"),
        ));
    }
    let addrs: Vec<SocketAddr> = (host, port as u16).to_socket_addrs()?.collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| addrs.first())
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "ホストを解決できません"))
}

/// VRChat の Chatbox へ OSC で送る。
pub struct VrcChatboxSender {
    socket: UdpSocket,
    target: SocketAddr,
    notification: bool,
    indicator: bool,
}

impl VrcChatboxSender {
    pub fn new(host: &str, port: i32, notification: bool, indicator: bool) -> io::Result<Self> {
        let target = resolve(host, port)?;
        let bind = if target.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        Ok(Self {
            socket: UdpSocket::bind(bind)?,
            target,
            notification,
            indicator,
        })
    }

    fn send_osc(&self, address: &str, args: &[OscArg]) -> io::Result<()> {
        self.socket
            .send_to(&encode_message(address, args), self.target)?;
        Ok(())
    }

    /// indicator が有効なら typing true → input → typing false の順に送る。
    pub fn send(&self, message: &str) -> io::Result<()> {
        if self.indicator {
            self.send_osc("/chatbox/typing", &[OscArg::Bool(true)])?;
        }
        self.send_osc(
            "/chatbox/input",
            &[
                OscArg::Str(message.to_string()),
                OscArg::Bool(true),
                OscArg::Bool(self.notification),
            ],
        )?;
        if self.indicator {
            self.send_osc("/chatbox/typing", &[OscArg::Bool(false)])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn 既知のバイト列と一致する() {
        let bytes = encode_message(
            "/chatbox/input",
            &[
                OscArg::Str("hi".into()),
                OscArg::Bool(true),
                OscArg::Bool(false),
            ],
        );
        let mut expected = Vec::new();
        expected.extend_from_slice(b"/chatbox/input\0\0");
        expected.extend_from_slice(b",sTF\0\0\0\0");
        expected.extend_from_slice(b"hi\0\0");
        assert_eq!(bytes, expected);
    }

    #[test]
    fn 長さが_4_の倍数の文字列は_4_バイト余分に埋まる() {
        let bytes = encode_message(
            "/a",
            &[OscArg::Str("abc".into()), OscArg::Str("abcd".into())],
        );
        let mut expected = Vec::new();
        expected.extend_from_slice(b"/a\0\0");
        expected.extend_from_slice(b",ss\0");
        expected.extend_from_slice(b"abc\0");
        expected.extend_from_slice(b"abcd\0\0\0\0");
        assert_eq!(bytes, expected);
    }

    #[test]
    fn int_はビッグエンディアン() {
        let bytes = encode_message("/i", &[OscArg::Int(0x0102_0304), OscArg::Int(-1)]);
        let mut expected = Vec::new();
        expected.extend_from_slice(b"/i\0\0");
        expected.extend_from_slice(b",ii\0");
        expected.extend_from_slice(&[1, 2, 3, 4, 0xff, 0xff, 0xff, 0xff]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn utf8_の文字列を送れる() {
        let bytes = encode_message("/x", &[OscArg::Str("🎵".into())]);
        // アドレス 4 + 型タグ 4 の後に、🎵（4 バイト）+ NUL 埋め 4 バイト
        assert_eq!(&bytes[8..], &[0xF0, 0x9F, 0x8E, 0xB5, 0, 0, 0, 0]);
    }

    fn receiver() -> (UdpSocket, i32) {
        let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
        rx.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let port = rx.local_addr().unwrap().port() as i32;
        (rx, port)
    }

    fn recv(rx: &UdpSocket) -> Vec<u8> {
        let mut buf = [0u8; 2048];
        let n = rx.recv(&mut buf).unwrap();
        buf[..n].to_vec()
    }

    #[test]
    fn udp_で_input_だけ送る() {
        let (rx, port) = receiver();
        let s = VrcChatboxSender::new("127.0.0.1", port, true, false).unwrap();
        s.send("hi").unwrap();
        let expected = encode_message(
            "/chatbox/input",
            &[
                OscArg::Str("hi".into()),
                OscArg::Bool(true),
                OscArg::Bool(true),
            ],
        );
        assert_eq!(recv(&rx), expected);
        // 追加のパケットは来ない
        rx.set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let mut buf = [0u8; 16];
        assert!(rx.recv(&mut buf).is_err());
    }

    #[test]
    fn udp_インジケーター有りは_3_パケットの順() {
        let (rx, port) = receiver();
        let s = VrcChatboxSender::new("localhost", port, false, true).unwrap();
        s.send("hi").unwrap();
        assert_eq!(
            recv(&rx),
            encode_message("/chatbox/typing", &[OscArg::Bool(true)])
        );
        assert_eq!(
            recv(&rx),
            encode_message(
                "/chatbox/input",
                &[
                    OscArg::Str("hi".into()),
                    OscArg::Bool(true),
                    OscArg::Bool(false)
                ]
            )
        );
        assert_eq!(
            recv(&rx),
            encode_message("/chatbox/typing", &[OscArg::Bool(false)])
        );
    }

    #[test]
    fn ポートが範囲外ならエラー() {
        assert!(VrcChatboxSender::new("127.0.0.1", -1, false, false).is_err());
        assert!(VrcChatboxSender::new("127.0.0.1", 65536, false, false).is_err());
    }
}
