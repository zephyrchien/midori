# TCP

TCP is used as the default transport protocol. No configuration is needed. And there is no other options for it.

On linux, zero-copy is enabled.

## position

endpoint->listen|remote->net->tcp

## example
use default value: net = tcp, transport = plain, tls = none
```json
{
  "endpoints": [
    {
      "listen": "0.0.0.0:5000",
      "remote": "example.com:443"
    }
  ]
}
```
equals
```json
{
  "endpoints": [
    {
      "listen": {
        "addr": "0.0.0.0:5000",
        "net": "tcp"
      },
      "remote": {
        "addr": "0.0.0.0:5000",
        "net": "tcp"
      }
    }
  ]
}
```
