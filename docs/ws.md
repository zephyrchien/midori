# WebSocket

WebSocket is established on top of an underlying connection, which could be TCP, UDS, or TLS.

## position
global->endpoints->endpoint->listen|remote->trans->ws

## options

### path: string
no default value

## example

ws over tcp
```shell
"remote": {
    "addr": "127.0.0.1:5000",
    "net": "tcp",
    "trans": {
        "proto": "ws",
        "path": "/test"
    }
}
```
```shell
"listen": {
    "addr": "127.0.0.1:5000",
    "net": "tcp",
    "trans": {
        "proto": "ws",
        "path": "/test"
    }
}
```

ws over uds
```shell
"remote": {
    "addr": "127.0.0.1:5000",
    "net": "uds",
    "trans": {
        "proto": "ws",
        "path": "/test"
    }
}
```

ws over tls
```shell
"remote": {
    "addr": "127.0.0.1:5000",
    "net": "tcp",
    "trans": {
        "proto": "ws",
        "path": "/test"
    },
    "tls": {
        // more details: docs/tls.md
        "skip_verify": true
    }
}
```