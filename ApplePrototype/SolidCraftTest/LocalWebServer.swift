import Foundation
import Network

/// A loopback-only origin lets bundled WebAssembly use normal fetch and persistent storage.
final class LocalWebServer {
    private var listener: NWListener?
    private let queue = DispatchQueue(label: "SolidCraft.loopback")
    private let root: URL
    static let port: UInt16 = 18784

    init(root: URL) { self.root = root }

    func start(completion: @escaping (Result<URL, Error>) -> Void) {
        do {
            let parameters = NWParameters.tcp
            parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: NWEndpoint.Port(rawValue: Self.port)!)
            let listener = try NWListener(using: parameters)
            self.listener = listener
            listener.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    if let url = URL(string: "http://127.0.0.1:\(Self.port)/index.html?sample&webgl&apple") {
                        DispatchQueue.main.async { completion(.success(url)) }
                    }
                case .failed(let error): DispatchQueue.main.async { completion(.failure(error)) }
                default: break
                }
            }
            listener.newConnectionHandler = { [weak self] in self?.accept($0) }
            listener.start(queue: queue)
        } catch { completion(.failure(error)) }
    }

    private func accept(_ connection: NWConnection) {
        connection.start(queue: queue)
        receive(connection, accumulated: Data())
    }

    private func receive(_ connection: NWConnection, accumulated: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 8192) { [weak self] bytes, _, complete, error in
            guard let self else { connection.cancel(); return }
            var request = accumulated
            if let bytes { request.append(bytes) }
            guard request.count <= 16384, error == nil else { connection.cancel(); return }
            if request.range(of: Data("\r\n\r\n".utf8)) != nil {
                self.respond(connection, request: request)
            } else if complete { connection.cancel() }
            else { self.receive(connection, accumulated: request) }
        }
    }

    private func respond(_ connection: NWConnection, request: Data) {
        let line = String(decoding: request, as: UTF8.self).components(separatedBy: "\r\n").first ?? ""
        let pieces = line.split(separator: " ")
        guard pieces.count >= 2, pieces[0] == "GET" else {
            send(connection, status: "405 Method Not Allowed", type: "text/plain", data: Data()); return
        }
        let rawPath = String(pieces[1]).components(separatedBy: "?").first ?? ""
        let path = rawPath.removingPercentEncoding ?? rawPath
        let relative = path == "/" ? "index.html" : String(path.dropFirst())
        let url = root.appendingPathComponent(relative).standardizedFileURL
        guard url.path.hasPrefix(root.standardizedFileURL.path + "/"),
              let data = try? Data(contentsOf: url) else {
            send(connection, status: "404 Not Found", type: "text/plain", data: Data()); return
        }
        // WebKit ignores a stylesheet served with any other type, leaving the canvas unsized.
        let types = ["html": "text/html; charset=utf-8", "js": "text/javascript", "wasm": "application/wasm", "ttf": "font/ttf", "json": "application/json",
                     "css": "text/css", "svg": "image/svg+xml", "png": "image/png", "webmanifest": "application/manifest+json"]
        send(connection, status: "200 OK", type: types[url.pathExtension] ?? "text/plain", data: data)
    }

    private func send(_ connection: NWConnection, status: String, type: String, data: Data) {
        var response = Data("HTTP/1.1 \(status)\r\nContent-Type: \(type)\r\nContent-Length: \(data.count)\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n".utf8)
        response.append(data)
        connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
    }
}
