# Midori

[![CI][ci-badge]][ci-url]
[![Codacy][codacy-badge]][codacy-url]
[![License][mit-badge]][mit-url]
![Activity][activity-img]

[ci-badge]: https://github.com/zephyrchien/midori/workflows/ci/badge.svg
[ci-url]: https://github.com/zephyrchien/midori/actions

[codacy-badge]: https://app.codacy.com/project/badge/Grade/908ed7e0dd5f4bec8984856931021165
[codacy-url]: https://www.codacy.com/gh/zephyrchien/midori/dashboard?utm_source=github.com&amp;utm_medium=referral&amp;utm_content=zephyrchien/midori&amp;utm_campaign=Badge_Grade

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/zephyrchien/midori/blob/master/LICENSE

[activity-img]: https://img.shields.io/github/commit-activity/m/zephyrchien/midori?color=green&label=commit

## Protocols
- [x] [TCP][tcp-doc-url]
- [x] [UDS][uds-doc-url]
- [ ] UDP
- [x] [TLS][tls-doc-url]
- [x] [WebSocket][ws-doc-url]
- [x] [HTTP2][h2-doc-url]
- [ ] gRPC
- [ ] QUIC

[doc-url]: https://github.com/zephyrchien/midori/tree/master/src

[tcp-doc-url]: https://github.com/zephyrchien/midori/blob/master/docs/tcp.md

[uds-doc-url]: https://github.com/zephyrchien/midori/blob/master/docs/uds.md

[tls-doc-url]: https://github.com/zephyrchien/midori/blob/master/docs/tls.md

[ws-doc-url]: https://github.com/zephyrchien/midori/blob/master/docs/ws.md

[h2-doc-url]: https://github.com/zephyrchien/midori/blob/master/docs/h2.md

## Usage
```bash
# start from a config file
# json (maybe toml will be supported in the future)
midori -c config.json
```

## Quick Start
Get started with a simple TCP relay(supports zero-copy on linux). First, write a config file:
```json
// config.json
{
    "endpoints":[
        {
            "listen": "0.0.0.0:5000",
            "remote": "1.2.3.4:8080"
        },
        {
            "listen": "0.0.0.0:10000",
            "remote": "www.example.com:443"
        },
    ]
}
```

Then launch these 2 endpoints:
```bash
midori -c config.json
```

#### Address Format
Almost all kinds of formats are supported, including `ipv4`, `ipv6`, `domain name`, `file path(recognized as uds)`

## Full Configure

Currently, the config file consists of `gloal params` and `endpoints`
```bash
{
    "dns_mode": "", // and other global params
    "endpoints": []
}
```

---
#### DNS Mode
The `trust-dns` crate supports different resolve strategies: `ipv4_only`, `ipv6_only`, **`ipv4_then_ipv6(default)`**, `ipv6_then_ipv4`, `ipv4_and_ipv6`.

---
#### Endpoint
In each endpoint, you need to specify the associated pair of `listen(server)` and `remote(client)`
```json
// endpoint
{
    "listen": "",
    "remote": ""
}
```

---
#### Endpoint Half
Below is the params of `listen` or `remote`. **Each field has a default value except for `addr`**. <br>
Moreover, `trans` and `tls` also support more complicated configuration options(e.g. `path`, `sni`, `ocsp`..). [Please check docs of each protocol for more details][doc-url].
```bash
// listen or remote
{
    "addr": "",  // must
    "net": "",  // tcp(deafult), uds, udp
    "trans": "",  // plain(default), ws, h2..
    "tls": ""  // none(default)
}
```

---
#### 
Finally, the config file *(json)* looks like:
```json
{
    "dns_mode": "ipv4_then_ipv6",
    "endpoints": [
        {
            "listen": {
                "addr": "0.0.0.0:5000",
                "net": "tcp",
                "trans": {
                    "proto": "ws",
                    "path": "/"
                },
                "tls": {
                    "cert": "x.crt",
                    "key": "x.pem",
                    "version": "tlsv1.3, tlsv1.2",
                    "apln": "http/1.1",
                    "ocsp": "x.ocsp"
                }
            },
            "remote": {
                "addr": "www.example.com:443",
                "net": "tcp",
                "trans": {
                    "proto": "h2",
                    "path": "/",
                    "server_push": false
                },
                "tls": {
                    "roots": "firefox",
                    "version": "tlsv1.3, tlsv1.2",
                    "sni": "www.example.com",
                    "apln": "h2",
                    "skip_verify": false,
                    "enable_sni": true,
                }
            }
        },
    ]
}
```

All the protocols can be applied to both sides of `listen(server)` and `remote(client)`. You could either use `net(tcp, uds, udp)` directly or combine them with `transport(ws, h2..)`.
| net | tcp | uds | udp |
| :---: | :---: | :---: | :---: |
| tls | O | O |
| ws | O | O |
| h2 | O | O |
| grpc | O | O |
| quic | | | O |

## License
[The MIT License (MIT)](https://github.com/zephyrchien/midori/blob/master/LICENSE)
