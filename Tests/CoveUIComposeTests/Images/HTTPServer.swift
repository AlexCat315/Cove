import Foundation
private import NIOCore
private import NIOHTTP1
private import NIOPosix

/// Local sockets exercise the production HTTP client on every desktop host.
final class ImageHTTPFixture: @unchecked Sendable {
    private let group = MultiThreadedEventLoopGroup(numberOfThreads: 1)
    private var listener: (any Channel)?
    private let lock = NSLock()
    private var slowStarted = false
    private var held: [any Channel] = []

    init() throws {
        do {
            listener = try ServerBootstrap(group: group)
                .serverChannelOption(ChannelOptions.socketOption(.so_reuseaddr), value: 1)
                .childChannelInitializer { [weak self] channel in
                    channel.pipeline.configureHTTPServerPipeline().flatMap {
                        channel.pipeline.addHandler(Handler(fixture: self))
                    }
                }.bind(host: "127.0.0.1", port: 0).wait()
        } catch {
            try? group.syncShutdownGracefully()
            throw error
        }
    }

    func url(_ path: String) -> URL {
        URL(string: "http://127.0.0.1:\(listener!.localAddress!.port!)/\(path)")!
    }

    func stop() {
        try? listener?.close().wait()
        listener = nil
        let channels = lock.withLock { let value = held; held = []; return value }
        channels.forEach { $0.close(promise: nil) }
        try? group.syncShutdownGracefully()
    }

    private func respond(to path: String, context: ChannelHandlerContext) {
        if path == "/slow" {
            lock.withLock { slowStarted = true; held.append(context.channel) }
            return
        }
        var status = HTTPResponseStatus.ok
        var headers = HTTPHeaders()
        let body: Data
        switch path {
        case "/slow-started":
            body = Data(lock.withLock { slowStarted ? "started" : "pending" }.utf8)
        case "/missing": status = .notFound; body = Data()
        case "/large": body = Data(repeating: UInt8(ascii: "x"), count: 2048)
        case "/vector", "/vector.jpg", "/fallback.svg":
            body = Data("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"160\" height=\"80\"><rect width=\"160\" height=\"80\" fill=\"red\"/></svg>".utf8)
            headers.add(name: "Content-Type", value: path == "/fallback.svg" ? "application/octet-stream" : "image/svg+xml; charset=utf-8")
        case "/redirect":
            status = .found; body = Data()
            headers.add(name: "Location", value: url("fallback.svg").absoluteString)
        default: body = Data("image data".utf8)
        }
        headers.add(name: "Content-Length", value: String(body.count))
        headers.add(name: "Connection", value: "close")
        let head = HTTPResponseHead(version: .http1_1, status: status, headers: headers)
        var buffer = context.channel.allocator.buffer(capacity: body.count)
        buffer.writeBytes(body)
        context.write(NIOAny(HTTPServerResponsePart.head(head)), promise: nil)
        context.write(NIOAny(HTTPServerResponsePart.body(.byteBuffer(buffer))), promise: nil)
        let channel = context.channel
        context.writeAndFlush(NIOAny(HTTPServerResponsePart.end(nil))).whenComplete { _ in
            channel.close(promise: nil)
        }
    }

    private final class Handler: ChannelInboundHandler, @unchecked Sendable {
        typealias InboundIn = HTTPServerRequestPart
        private weak var fixture: ImageHTTPFixture?
        private var path = "/"
        init(fixture: ImageHTTPFixture?) { self.fixture = fixture }
        func channelRead(context: ChannelHandlerContext, data: NIOAny) {
            switch unwrapInboundIn(data) {
            case .head(let head): path = String(head.uri.split(separator: "?").first ?? "/")
            case .end: fixture?.respond(to: path, context: context)
            case .body: break
            }
        }
    }
}
