# QUIC

QUIC is a morden protocol built on top of udp, which has these well known benifits:
- Fast: Only requires 1 rtt to setup. Also, `0-rtt` is supported.
- Secure: Packets are encrypted by tls1.3, which only use `AES_128_GCM_SHA256`,`AES_256_GCM_SHA384` and `CHACHA20_POLY1305_SHA256` as cipher suits.
- Multiplex: Multiple streams over one connection. Compared to HTTP2, there is no head-of-line blocking issue.

## position
```shell
endpoint->listen|remote->net=udp (*must*)
endpoint->listen|remote->tls (*must*)
endpoint->listen|remote->trans->quic
```

## options

### mux(client): int
max number of concurrent streams for each connection.

## example

Also See: [TLS][tls-doc-url]

[tls-doc-url]: https://github.com/zephyrchien/midori/blob/master/docs/tls.md

```shell
"remote": {
    "addr": "127.0.0.1:5000",
    "net": "udp",
    "trans": {
        "proto": "quic",
        "mux": 8
    },
    "tls": {
        "sni": "example.com",
        "roots": "firefox",
        "early_data": true,
    }
}
```

```shell
"listen": {
    "addr": "127.0.0.1:5000",
    "net": "udp",
    "trans": {
        "proto": "quic"
    },
    "tls": {
        "cert": "cert.pem",
        "key": "key.pem"
    }
}
```