# UDP

UDP is not well supported so far.

By design, the server side behaves differently from the client side:

The server binds a given port with `SO_REUSEADDR` socket opt. When a packet from new address is coming, the server creates a new `UdpSocket(reuse addr)` and connect it with the new address. Afterwards the packets from the same address will be handled by this socket.

The client side simply uses the specified address as sending destination, it does not limit where the packet comes from. It is `Full Cone`.

## position
endpoint->listen|remote->net->udp

## example
```json
{
  "endpoints": [
    {
      "listen": {
        "addr": "0.0.0.0:5000",
        "net": "udp"
      },
      "remote": {
        "addr": "127.0.0.1:10000",
        "net": "udp"
      }
    }
  ]
}
```