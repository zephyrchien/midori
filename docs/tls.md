# TLS

The TLS module is powered by Rustls, instead of Openssl.

## position
endpoint->listen|remote->tls

## options

### versions: string
default: "tlsv1.3, tlsv1.2"

### alpns: string
default: "h2, http/1.1"

### sni(client): string
default: $addr

### enable_sni(client): bool
default: true

### skip_verify(client): bool
default: false

### enable_early_data(client): bool
default: false

### roots(client): string
root certificates.

possible values:
- "native" // use system's roots
- "firefox" // use firefox's roots
- "file path"  // specify a custom CA file

default: "firefox"

### cert(server): string
certificate path, no default value

### key(server): string
private key path, no default value

**rustls does not support legacy EC private keys. You must convert it into pkcs8 format:**
```shell
openssl pkcs8 -topk8 -nocrypt -in <old> -out <new>
```

### self-signed-certificate:
generate certificate if **cert path equals key path(cert == key)**. $cert or $key is used as common name(CN).

### ocsp(server): string
specify the ocsp file, which enables ocsp stapling.

default: "", and this feature is disabled

the ocsp file could be created with openssl:
```shell
// for let's encrypt
openssl ocsp -issuer <ca> -cert <cert> -url http://r3.o.lencr.org -header Host=r3.o.lencr.org -respout <output> -noverify -no_nonce
```

## example
```shell
"listen": {
    "addr": "127.0.0.1:5000",
    "net": "tcp",  // could be omitted
    "trans": {
        "proto": "plain"  // could be omitted
    },
    "tls": {
        "versions": "tlsv1.3, tlsv1.2",
        "alpns": "h2, http/1.1",
        "key": "localhost",
        "cert": "localhost"  // generate certificate
    }
}
```

```shell
"remote": {
    "addr": "127.0.0.1:5000",
    "net": "tcp",  // could be omitted
    "trans": {
        "proto": "plain"  // could be omitted
    },
    "tls": {
        "versions": "tlsv1.3",
        "sni": "localhost",
        "alpns": "h2",
        "enable_early_data": true,
        "skip_verify": true
    }
}
```
