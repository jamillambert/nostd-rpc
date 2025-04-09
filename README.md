## nostd-rpc
WIP:

Sends http requests using [smoltcp](https://github.com/smoltcp-rs/smoltcp)

Requires a TAP device called `tap0` which can be set up as shown below:
```
sudo ip tuntap add dev tap0 mode tap user $USER
sudo ip link set tap0 up
sudo ip addr add 192.168.42.100/24 dev tap0
sudo iptables -t nat -A POSTROUTING -s 192.168.42.0/24 -j MASQUERADE
sudo sysctl net.ipv4.ip_forward=1 > /dev/null
```
