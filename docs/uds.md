# Unix Domain Socket

It is only for Unix. Although Windows 10 has supported UDS recently, and there has been a nice rust library: [tokio-uds-windows](https://github.com/Azure/tokio-uds-windows)

On linux, zero-copy is enabled.

## position
endpoint->listen|remote->net->uds

## example
use default value: transport = plain, tls = none
```json
{
  "endpoints": [
    {
      "listen": {
        "addr": "/home/midori/local.sock",
        "net": "uds"
      },
      "remote": {
        "addr": "/home/midori/remote.sock",
        "net": "uds"
      }
    }
  ]
}
```
