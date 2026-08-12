use std::{
    collections::HashSet,
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    time::Duration,
};

use reqwest::{redirect::Policy, Url};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustedHttpPolicy {
    RemoteMeeting,
    AiProvider,
}

#[derive(Debug, Clone)]
pub struct TrustedHttpTarget {
    base_url: Url,
    dns_override: Option<(String, Vec<SocketAddr>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedHttpError {
    message: &'static str,
}

impl fmt::Display for TrustedHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for TrustedHttpError {}

impl TrustedHttpError {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl TrustedHttpTarget {
    pub fn resolve(value: &str, policy: TrustedHttpPolicy) -> Result<Self, TrustedHttpError> {
        let mut base_url = Url::parse(value.trim())
            .map_err(|_| TrustedHttpError::new("目标地址不是有效 URL。"))?;
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(TrustedHttpError::new("目标地址只支持 HTTP(S) URL。"));
        }
        if !base_url.username().is_empty() || base_url.password().is_some() {
            return Err(TrustedHttpError::new("目标地址不能内嵌用户名或密码。"));
        }
        if base_url.host_str().is_none() {
            return Err(TrustedHttpError::new("目标地址缺少主机名。"));
        }
        if policy == TrustedHttpPolicy::AiProvider
            && (base_url.query().is_some() || base_url.fragment().is_some())
        {
            return Err(TrustedHttpError::new(
                "AI 接口地址不能包含 query 或 fragment。",
            ));
        }
        if policy == TrustedHttpPolicy::RemoteMeeting {
            base_url.set_query(None);
            base_url.set_fragment(None);
        }

        let host = base_url
            .host_str()
            .ok_or_else(|| TrustedHttpError::new("目标地址缺少主机名。"))?
            .to_string();
        if let Some(literal_ip) = literal_host_ip(&base_url) {
            validate_destination(&base_url, &[literal_ip])?;
            return Ok(Self {
                base_url,
                dns_override: None,
            });
        }
        if base_url.scheme() != "https" {
            return Err(TrustedHttpError::new(
                "HTTP 只允许 IP 字面量 loopback 本机端点。",
            ));
        }

        let port = base_url
            .port_or_known_default()
            .ok_or_else(|| TrustedHttpError::new("目标地址缺少有效端口。"))?;
        let addresses = resolve_domain_addresses(&host, port)?;
        let resolved_ips = addresses
            .iter()
            .map(|address| address.ip())
            .collect::<Vec<_>>();
        validate_destination(&base_url, &resolved_ips)?;

        Ok(Self {
            base_url,
            dns_override: Some((host, addresses)),
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn endpoint(&self, path: &[&str]) -> Result<Url, TrustedHttpError> {
        let mut url = self.base_url.clone();
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| TrustedHttpError::new("目标地址不能作为 API 基础地址。"))?;
        segments.pop_if_empty();
        for segment in path {
            segments.push(segment);
        }
        drop(segments);
        Ok(url)
    }

    pub fn blocking_client(
        &self,
        timeout: Duration,
    ) -> Result<reqwest::blocking::Client, TrustedHttpError> {
        let mut builder = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .redirect(Policy::none())
            .no_proxy()
            .https_only(self.base_url.scheme() == "https");
        if let Some((domain, addresses)) = self.dns_override.as_ref() {
            builder = builder.resolve_to_addrs(domain, addresses);
        }
        builder
            .build()
            .map_err(|_| TrustedHttpError::new("初始化受信任 HTTP 客户端失败。"))
    }

    pub fn async_client(
        &self,
        connect_timeout: Duration,
        timeout: Duration,
    ) -> Result<reqwest::Client, TrustedHttpError> {
        let mut builder = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .redirect(Policy::none())
            .no_proxy()
            .https_only(self.base_url.scheme() == "https");
        if let Some((domain, addresses)) = self.dns_override.as_ref() {
            builder = builder.resolve_to_addrs(domain, addresses);
        }
        builder
            .build()
            .map_err(|_| TrustedHttpError::new("初始化受信任 HTTP 客户端失败。"))
    }
}

fn resolve_domain_addresses(host: &str, port: u16) -> Result<Vec<SocketAddr>, TrustedHttpError> {
    let resolved = (host, port)
        .to_socket_addrs()
        .map_err(|_| TrustedHttpError::new("解析目标域名失败。"))?;
    let mut seen = HashSet::new();
    let addresses = resolved
        .filter(|address| seen.insert(*address))
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        Err(TrustedHttpError::new("目标域名未解析到任何地址。"))
    } else {
        Ok(addresses)
    }
}

fn validate_destination(base_url: &Url, resolved_ips: &[IpAddr]) -> Result<(), TrustedHttpError> {
    if let Some(literal_ip) = literal_host_ip(base_url) {
        if is_loopback_ip(literal_ip) {
            return Ok(());
        }
        if base_url.scheme() != "https" {
            return Err(TrustedHttpError::new(
                "禁止通过非 loopback HTTP 端点传输凭据。",
            ));
        }
        if !is_public_remote_ip(literal_ip) {
            return Err(TrustedHttpError::new("目标地址指向受限网络。"));
        }
        return Ok(());
    }

    if base_url.scheme() != "https" {
        return Err(TrustedHttpError::new(
            "HTTP 只允许 IP 字面量 loopback 本机端点。",
        ));
    }
    if resolved_ips.is_empty() {
        return Err(TrustedHttpError::new("目标域名缺少已验证的解析地址。"));
    }
    if resolved_ips.iter().any(|ip| !is_public_remote_ip(*ip)) {
        return Err(TrustedHttpError::new(
            "目标域名解析到私网、loopback、link-local、metadata 或保留地址。",
        ));
    }
    Ok(())
}

fn literal_host_ip(url: &Url) -> Option<IpAddr> {
    let host = url.host_str()?;
    host.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

fn is_loopback_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || mapped_ipv4(ip)
                    .map(|mapped| mapped.is_loopback())
                    .unwrap_or(false)
        }
    }
}

fn is_public_remote_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => mapped_ipv4(ip)
            .map(is_public_ipv4)
            .unwrap_or_else(|| is_public_ipv6(ip)),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [first, second, third, fourth] = ip.octets();
    if first == 0
        || first == 10
        || first == 127
        || first >= 224
        || (first == 100 && (64..=127).contains(&second))
        || (first == 169 && second == 254)
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 168)
        || (first == 192 && second == 0 && third == 0)
        || (first == 192 && second == 0 && third == 2)
        || (first == 192 && second == 88 && third == 99)
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
    {
        return false;
    }

    [first, second, third, fourth] != [168, 63, 129, 16]
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    if segments[0] & 0xe000 != 0x2000 {
        return false;
    }

    let special_2001 = segments[0] == 0x2001
        && (segments[1] == 0
            || (segments[1] == 2 && segments[2] == 0)
            || (segments[1] & 0xfff0) == 0x0010
            || (segments[1] & 0xfff0) == 0x0020
            || segments[1] == 0x0db8);
    let documentation_3fff = segments[0] == 0x3fff && (segments[1] & 0xf000) == 0;
    !special_2001 && segments[0] != 0x2002 && !documentation_3fff
}

fn mapped_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let segments = ip.segments();
    if segments[0] == 0
        && segments[1] == 0
        && segments[2] == 0
        && segments[3] == 0
        && segments[4] == 0
        && segments[5] == 0xffff
    {
        Some(Ipv4Addr::new(
            (segments[6] >> 8) as u8,
            segments[6] as u8,
            (segments[7] >> 8) as u8,
            segments[7] as u8,
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_public_https_and_literal_loopback_http() {
        for value in [
            "https://8.8.8.8/v1",
            "http://127.0.0.1:8787",
            "http://[::1]:8787",
        ] {
            assert!(TrustedHttpTarget::resolve(value, TrustedHttpPolicy::AiProvider).is_ok());
        }
    }

    #[test]
    fn rejects_credentials_query_fragments_and_non_http_schemes_for_ai() {
        for value in [
            "https://user:pass@8.8.8.8/v1",
            "https://8.8.8.8/v1?token=secret",
            "https://8.8.8.8/v1#debug",
            "file:///tmp/provider",
        ] {
            assert!(TrustedHttpTarget::resolve(value, TrustedHttpPolicy::AiProvider).is_err());
        }
    }

    #[test]
    fn remote_policy_keeps_legacy_query_and_fragment_normalization() {
        let target = TrustedHttpTarget::resolve(
            "https://8.8.8.8/liberty/?debug=1#fragment",
            TrustedHttpPolicy::RemoteMeeting,
        )
        .expect("remote target");
        assert_eq!(target.base_url().as_str(), "https://8.8.8.8/liberty/");
    }

    #[test]
    fn rejects_private_link_local_metadata_and_mapped_addresses() {
        for value in [
            "https://10.0.0.1",
            "https://100.100.100.200",
            "https://169.254.169.254",
            "https://168.63.129.16",
            "https://192.168.1.1",
            "https://224.0.0.1",
            "https://[fd00::1]",
            "https://[fe80::1]",
            "https://[2001:db8::1]",
            "https://[::ffff:10.0.0.1]",
        ] {
            assert!(
                TrustedHttpTarget::resolve(value, TrustedHttpPolicy::AiProvider).is_err(),
                "accepted blocked address {value}"
            );
        }
    }

    #[test]
    fn rejects_domain_answers_with_private_or_mixed_addresses() {
        let base_url = Url::parse("https://api.example.com").expect("URL");
        let public = [
            "8.8.8.8".parse::<IpAddr>().unwrap(),
            "2606:4700:4700::1111".parse::<IpAddr>().unwrap(),
        ];
        assert!(validate_destination(&base_url, &public).is_ok());

        let mixed = [
            "8.8.8.8".parse::<IpAddr>().unwrap(),
            "10.0.0.1".parse::<IpAddr>().unwrap(),
        ];
        assert!(validate_destination(&base_url, &mixed).is_err());
    }

    #[test]
    fn endpoint_preserves_base_path_and_encodes_segments() {
        let target = TrustedHttpTarget::resolve(
            "https://8.8.8.8/liberty/",
            TrustedHttpPolicy::RemoteMeeting,
        )
        .expect("target");
        assert_eq!(
            target
                .endpoint(&["api", "jobs", "job / one"])
                .expect("endpoint")
                .as_str(),
            "https://8.8.8.8/liberty/api/jobs/job%20%2F%20one"
        );
    }
}
