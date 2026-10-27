import SwiftUI
import WebKit
import Combine
import UniformTypeIdentifiers

@main
struct SolidcraftForIPadApp: App {
    @StateObject private var model = CADModel()
    var body: some Scene {
        WindowGroup { CADView(model: model) }
    }
}

struct CADView: View {
    @ObservedObject var model: CADModel
    var body: some View {
        CADWebView(webView: model.webView)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .ignoresSafeArea(.container, edges: [.horizontal, .bottom])
    }
}

struct CADWebView: UIViewRepresentable {
    let webView: WKWebView
    func makeUIView(context: Context) -> WKWebView { webView }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}

@MainActor
final class CADModel: NSObject, ObservableObject, WKScriptMessageHandler, WKNavigationDelegate, WKUIDelegate, UIDocumentPickerDelegate, UIGestureRecognizerDelegate {
    @Published var status = "Starting…" { didSet { updateRuntimeAccessibility() } }
    @Published var diagnostics = "" { didSet { updateRuntimeAccessibility() } }
    @Published var layoutDetails = "" { didSet { updateRuntimeAccessibility() } }
    @Published var exportURL: URL?
    let webView: WKWebView
    private var server: LocalWebServer?

    private func updateRuntimeAccessibility() {
        webView.accessibilityValue = status + "\n" + diagnostics + "\nLayout: " + layoutDetails
    }

    override init() {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .default()
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = true
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()
        configuration.userContentController.add(self, name: "solidcraft")
        #if targetEnvironment(macCatalyst)
        var pencilPolicy = "window.solidcraftRequirePencil = false;"
        #else
        var pencilPolicy = "window.solidcraftRequirePencil = true;"
        #endif
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-test") {
            pencilPolicy += "window.solidcraftUITestProbe = true;"
        }
        #endif
        configuration.userContentController.addUserScript(WKUserScript(source: pencilPolicy, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        for name in ["input", "bridge"] {
            if let url = Bundle.main.url(forResource: name, withExtension: "js"), let script = try? String(contentsOf: url, encoding: .utf8) {
                configuration.userContentController.addUserScript(WKUserScript(source: script, injectionTime: .atDocumentStart, forMainFrameOnly: true))
            }
        }
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.isInspectable = true
        webView.scrollView.isScrollEnabled = false
        webView.scrollView.bounces = false
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.isOpaque = false
        webView.accessibilityIdentifier = "cadWebView"
        #if !targetEnvironment(macCatalyst)
        let pencilHover = UIHoverGestureRecognizer(target: self, action: #selector(handlePencilHover(_:)))
        pencilHover.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.pencil.rawValue)]
        pencilHover.delegate = self
        pencilHover.cancelsTouchesInView = false
        webView.addGestureRecognizer(pencilHover)
        #endif
        guard let root = Bundle.main.url(forResource: "Web", withExtension: nil) else {
            status = "Missing CAD assets"; return
        }
        server = LocalWebServer(root: root)
        server?.start { [weak self] result in
            switch result {
            case .success(let url): self?.webView.load(URLRequest(url: url))
            case .failure(let error): self?.status = "Startup failed"; self?.diagnostics = error.localizedDescription
            }
        }
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        gestureRecognizer is UIHoverGestureRecognizer || otherGestureRecognizer is UIHoverGestureRecognizer
    }

    #if !targetEnvironment(macCatalyst)
    @objc private func handlePencilHover(_ recognizer: UIHoverGestureRecognizer) {
        let point = recognizer.location(in: webView)
        guard point.x.isFinite, point.y.isFinite else { return }
        let ended = recognizer.state == .ended || recognizer.state == .cancelled
        webView.evaluateJavaScript("window.solidcraftPencilHover?.(\(point.x), \(point.y), \(ended))", completionHandler: nil)
    }
    #endif

    private func presentFilePicker() {
        guard let scene = webView.window?.windowScene,
              var presenter = scene.windows.first(where: { $0.isKeyWindow })?.rootViewController else {
            finishFilePicker(name: nil, data: nil, error: "Cannot present Files right now")
            return
        }
        while let next = presenter.presentedViewController { presenter = next }
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.data], asCopy: true)
        picker.allowsMultipleSelection = false
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--ui-test") {
            picker.directoryURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
        }
        #endif
        picker.delegate = self
        presenter.present(picker, animated: true)
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
        finishFilePicker(name: nil, data: nil, error: nil)
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        guard let url = urls.first else { finishFilePicker(name: nil, data: nil, error: nil); return }
        let accessing = url.startAccessingSecurityScopedResource()
        defer { if accessing { url.stopAccessingSecurityScopedResource() } }
        do {
            let values = try url.resourceValues(forKeys: [.fileSizeKey])
            guard let size = values.fileSize, size <= 128 * 1024 * 1024 else {
                finishFilePicker(name: nil, data: nil, error: "File exceeds the 128 MB import limit")
                return
            }
            let data = try Data(contentsOf: url, options: .mappedIfSafe)
            finishFilePicker(name: url.lastPathComponent, data: data, error: nil)
        } catch {
            finishFilePicker(name: nil, data: nil, error: "Could not read the selected file")
        }
    }

    private func finishFilePicker(name: String?, data: Data?, error: String?) {
        webView.callAsyncJavaScript(
            "window.solidcraftPickedFile(name, base64, error)",
            arguments: ["name": name as Any? ?? NSNull(), "base64": data?.base64EncodedString() ?? "", "error": error as Any? ?? NSNull()],
            in: nil, in: .page) { _ in }
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? [String: Any], let kind = body["kind"] as? String else { return }
        switch kind {
        case "pickFile":
            presentFilePicker()
        case "ready":
            status = "CAD ready · offline"
            diagnostics = body["text"] as? String ?? ""
            writeDiagnostic()
            #if DEBUG
            if ProcessInfo.processInfo.arguments.contains("--workflow-test") {
                webView.callAsyncJavaScript("return await window.solidcraftWorkflowCheck()", arguments: [:], in: nil, in: .page) { _ in }
            }
            #endif
        case "error":
            status = "CAD error"
            diagnostics = body["text"] as? String ?? "Unknown error"
            writeDiagnostic()
        case "hover":
            if let text = body["text"] as? String {
                if let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
                    try? text.write(to: documents.appendingPathComponent("pencil-hover-check.txt"), atomically: true, encoding: .utf8)
                }
            }
        case "navigation":
            if let text = body["text"] as? String {
                diagnostics = (diagnostics.components(separatedBy: "\n").first ?? "") + "\n" + text
                writeDiagnostic()
            }
        case "layout":
            layoutDetails = body["text"] as? String ?? ""

        case "export":
            guard let name = body["name"] as? String, let encoded = body["base64"] as? String,
                  encoded.count <= 180_000_000, let data = Data(base64Encoded: encoded) else { return }
            do {
                let documents = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let exports = documents.appendingPathComponent("Exports", isDirectory: true)
                try FileManager.default.createDirectory(at: exports, withIntermediateDirectories: true)
                let filename = URL(fileURLWithPath: name).lastPathComponent
                guard !filename.isEmpty, filename != ".", filename != ".." else { return }
                let folder = exports.appendingPathComponent(UUID().uuidString, isDirectory: true)
                try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
                let destination = folder.appendingPathComponent(filename)
                try data.write(to: destination, options: .atomic)
                exportURL = destination
                status = "Export saved · \(filename)"
            } catch { status = "Export failed"; diagnostics = error.localizedDescription }
        default: break
        }
    }

    private func writeDiagnostic() {
        if let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
            try? (status + "\n" + diagnostics).write(to: documents.appendingPathComponent("runtime-check.txt"), atomically: true, encoding: .utf8)
        }
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        status = "Load failed"; diagnostics = error.localizedDescription
    }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        status = "CAD process stopped"; diagnostics = "Reopen the app to recover saved work."
        writeDiagnostic()
    }
    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        guard let url = action.request.url else { decisionHandler(.cancel); return }
        if url.host == "127.0.0.1", url.port == Int(LocalWebServer.port) { decisionHandler(.allow) }
        else { decisionHandler(.cancel) }
    }
    func webView(_ webView: WKWebView, runJavaScriptAlertPanelWithMessage message: String, initiatedByFrame frame: WKFrameInfo, completionHandler: @escaping () -> Void) {
        status = message; completionHandler()
    }
}
