#!/usr/bin/env python3
"""Linux port of MehrON's wrong-sequence fake ClientHello injector.

It observes IPv4/TCP handshake copies with AF_PACKET and injects one valid TLS
ClientHello whose SNI is fake, using a deliberately out-of-window sequence.
The real TLS stream remains untouched and flows through the local TCP relay.
"""
import asyncio
import json
import os
import socket
import ssl
import struct
import sys
import threading
import time


def checksum(data):
    if len(data) & 1:
        data += b"\0"
    total = sum(struct.unpack("!%dH" % (len(data) // 2), data))
    total = (total & 0xffff) + (total >> 16)
    total = (total & 0xffff) + (total >> 16)
    return (~total) & 0xffff


def fake_client_hello(server_name):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    incoming, outgoing = ssl.MemoryBIO(), ssl.MemoryBIO()
    client = context.wrap_bio(incoming, outgoing, server_side=False, server_hostname=server_name)
    try:
        client.do_handshake()
    except (ssl.SSLWantReadError, ssl.SSLWantWriteError):
        pass
    return outgoing.read()


def parse_ipv4_tcp(frame):
    if len(frame) < 14:
        return None
    offset = 14
    ethertype = struct.unpack_from("!H", frame, 12)[0]
    while ethertype in (0x8100, 0x88a8, 0x9100):
        if len(frame) < offset + 4:
            return None
        ethertype = struct.unpack_from("!H", frame, offset + 2)[0]
        offset += 4
    if ethertype != 0x0800 or len(frame) < offset + 40:
        return None
    version_ihl = frame[offset]
    if version_ihl >> 4 != 4:
        return None
    ihl = (version_ihl & 0x0f) * 4
    if ihl < 20 or frame[offset + 9] != 6 or len(frame) < offset + ihl + 20:
        return None
    total_len = struct.unpack_from("!H", frame, offset + 2)[0]
    tcp = offset + ihl
    tcp_hlen = (frame[tcp + 12] >> 4) * 4
    if tcp_hlen < 20 or len(frame) < tcp + tcp_hlen:
        return None
    src = socket.inet_ntoa(frame[offset + 12:offset + 16])
    dst = socket.inet_ntoa(frame[offset + 16:offset + 20])
    sport, dport = struct.unpack_from("!HH", frame, tcp)
    seq, ack = struct.unpack_from("!II", frame, tcp + 4)
    flags = frame[tcp + 13]
    payload_len = max(0, min(total_len, len(frame) - offset) - ihl - tcp_hlen)
    return src, dst, sport, dport, seq, ack, flags, payload_len


def make_ipv4_tcp(src, dst, sport, dport, seq, ack, payload):
    src_b, dst_b = socket.inet_aton(src), socket.inet_aton(dst)
    tcp_len = 20 + len(payload)
    tcp = struct.pack("!HHII", sport, dport, seq & 0xffffffff, ack & 0xffffffff)
    tcp += bytes((5 << 4, 0x18)) + struct.pack("!HHH", 65535, 0, 0) + payload
    pseudo = src_b + dst_b + bytes((0, 6)) + struct.pack("!H", tcp_len)
    tcp_sum = checksum(pseudo + tcp)
    tcp = tcp[:16] + struct.pack("!H", tcp_sum) + tcp[18:]
    ip_len = 20 + tcp_len
    ident = int.from_bytes(os.urandom(2), "big")
    ip = struct.pack("!BBHHHBBH4s4s", 0x45, 0, ip_len, ident, 0x4000, 64, 6, 0, src_b, dst_b)
    ip = ip[:10] + struct.pack("!H", checksum(ip)) + ip[12:]
    return ip + tcp


class Injector:
    def __init__(self, target_ip, target_port, fake_sni):
        self.target_ip, self.target_port = target_ip, target_port
        self.fake_data = fake_client_hello(fake_sni)
        self.flows = {}
        self.lock = threading.Lock()
        self.send_lock = threading.Lock()
        self.raw_send = socket.socket(socket.AF_INET, socket.SOCK_RAW, socket.IPPROTO_RAW)
        self.raw_send.setsockopt(socket.IPPROTO_IP, socket.IP_HDRINCL, 1)
        self.capture = socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.htons(0x0800))
        self.capture.settimeout(1.0)
        self.running = True

    def add_flow(self, local_ip, local_port):
        key = (local_ip, local_port, self.target_ip, self.target_port)
        with self.lock:
            self.flows[key] = {"syn": None, "synack": None, "sent": False}
        return key

    def remove_flow(self, key):
        with self.lock:
            self.flows.pop(key, None)

    def run(self):
        while self.running:
            try:
                packet = parse_ipv4_tcp(self.capture.recv(65575))
            except socket.timeout:
                continue
            except OSError:
                break
            if not packet:
                continue
            src, dst, sport, dport, seq, ack, flags, payload_len = packet
            syn, ack_flag, rst, fin = bool(flags & 0x02), bool(flags & 0x10), bool(flags & 0x04), bool(flags & 0x01)
            if rst or fin:
                continue
            outbound = (src, sport, dst, dport)
            inbound = (dst, dport, src, sport)
            with self.lock:
                flow = self.flows.get(outbound) or self.flows.get(inbound)
                if flow is None:
                    continue
                key = outbound if outbound in self.flows else inbound
                local_ip, local_port, remote_ip, remote_port = key
                if src == local_ip and syn and not ack_flag and payload_len == 0:
                    flow["syn"] = seq
                    continue
                if src == remote_ip and syn and ack_flag and flow["syn"] is not None and ack == ((flow["syn"] + 1) & 0xffffffff):
                    flow["synack"] = seq
                    continue
                if src != local_ip or not ack_flag or syn or flow["syn"] is None or flow["synack"] is None or flow["sent"]:
                    continue
                if seq != ((flow["syn"] + 1) & 0xffffffff) or ack != ((flow["synack"] + 1) & 0xffffffff):
                    continue
                flow["sent"] = True
                fake = make_ipv4_tcp(local_ip, remote_ip, local_port, remote_port,
                                     flow["syn"] + 1 - len(self.fake_data), flow["synack"] + 1, self.fake_data)
            time.sleep(0.001)
            try:
                with self.send_lock:
                    self.raw_send.sendto(fake, (remote_ip, remote_port))
                print(f"fake ClientHello injected for {remote_ip}:{remote_port}", flush=True)
            except OSError as exc:
                print(f"SNI injection failed: {exc}", file=sys.stderr, flush=True)

    def close(self):
        self.running = False
        try:
            self.capture.close()
            self.raw_send.close()
        except OSError:
            pass


async def relay(reader, writer, injector, target):
    local = None
    key = None
    peer = writer.get_extra_info("peername")
    try:
        route = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        route.connect(target)
        local_ip = route.getsockname()[0]
        route.close()
        local = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        local.setblocking(False)
        local.bind((local_ip, 0))
        key = injector.add_flow(local_ip, local.getsockname()[1])
        await asyncio.wait_for(asyncio.get_running_loop().sock_connect(local, target), 5)
        remote_reader, remote_writer = await asyncio.open_connection(sock=local)
        local = None
        async def pump(source, destination):
            while True:
                chunk = await source.read(65536)
                if not chunk:
                    break
                destination.write(chunk)
                await destination.drain()
            destination.write_eof()
        await asyncio.gather(pump(reader, remote_writer), pump(remote_reader, writer))
        remote_writer.close()
        await remote_writer.wait_closed()
    except Exception as exc:
        print(f"relay {peer}: {exc}", file=sys.stderr, flush=True)
    finally:
        if key:
            injector.remove_flow(key)
        if local:
            local.close()
        writer.close()
        try:
            await writer.wait_closed()
        except Exception:
            pass


async def main():
    with open(sys.argv[1], encoding="utf-8") as stream:
        config = json.load(stream)
    target_ip = config["CONNECT_IP"]
    if ":" in target_ip:
        raise RuntimeError("Linux SNI injector currently requires an IPv4 target")
    target = (target_ip, int(config["CONNECT_PORT"]))
    injector = Injector(target_ip, target[1], config["FAKE_SNI"])
    thread = threading.Thread(target=injector.run, daemon=True)
    thread.start()
    server = await asyncio.start_server(lambda r, w: relay(r, w, injector, target),
                                        config.get("LISTEN_HOST", "127.0.0.1"), int(config["LISTEN_PORT"]))
    print(f"SNI spoof relay listening on {config.get('LISTEN_HOST', '127.0.0.1')}:{config['LISTEN_PORT']} → {target_ip}:{target[1]}", flush=True)
    async with server:
        try:
            await server.serve_forever()
        finally:
            injector.close()
            thread.join(timeout=2)


if __name__ == "__main__":
    asyncio.run(main())
