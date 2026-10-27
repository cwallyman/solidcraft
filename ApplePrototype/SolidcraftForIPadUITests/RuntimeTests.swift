import XCTest
import UIKit

final class RuntimeTests: XCTestCase {
    func testFingerInterfaceBrowserResizeCollapseAndFaceAlignment() {
        XCUIDevice.shared.orientation = .landscapeRight
        let app = XCUIApplication()
        app.launchArguments = ["--ui-test"]
        app.launch()
        let web = app.webViews["cadWebView"]
        func snapshot() -> [String: Any] {
            let value = web.value as? String ?? ""
            guard let text = value.components(separatedBy: "\nLayout: ").last,
                  let data = text.data(using: .utf8),
                  let layout = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
            return layout
        }
        func wait(_ condition: @escaping () -> Bool) {
            XCTAssertTrue(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)], timeout: 15) == .completed, web.value as? String ?? "No layout")
        }
        func rect(_ key: String) -> CGRect {
            let values = (snapshot()["rects"] as? [String: Any])?[key] as? [NSNumber] ?? []
            XCTAssertEqual(values.count,4,key)
            guard values.count == 4 else { return .zero }
            let scale = (snapshot()["zoom"] as? NSNumber)?.doubleValue ?? 1
            return CGRect(x: values[0].doubleValue*scale,y:values[1].doubleValue*scale,width:values[2].doubleValue*scale,height:values[3].doubleValue*scale)
        }
        func coordinate(_ p: CGPoint) -> XCUICoordinate {
            web.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx:p.x,dy:p.y))
        }
        func record(_ name: String) {
            let shot = XCTAttachment(screenshot:XCUIScreen.main.screenshot())
            shot.name=name; shot.lifetime = .keepAlways; add(shot)
        }
        wait { snapshot()["qa_face"] is [NSNumber] }
        record("Touch layout with view controls at upper right")
        let panel = rect("browser:panel")
        let divider = CGPoint(x:panel.maxX-1,y:panel.midY)
        coordinate(divider).press(forDuration:0.15,thenDragTo:coordinate(CGPoint(x:divider.x+100,y:divider.y)))
        wait { rect("browser:panel").width > panel.width+60 }
        record("Browser divider resized with a finger")
        let toggle = rect("browser:toggle")
        coordinate(CGPoint(x:toggle.midX,y:toggle.midY)).tap()
        wait { snapshot()["browser_collapsed"] as? Bool == true }
        XCTAssertLessThan(rect("browser:panel").width,60*((snapshot()["zoom"] as? NSNumber)?.doubleValue ?? 1))
        record("Browser collapsed to folder B")
        let reopen = rect("browser:toggle")
        coordinate(CGPoint(x:reopen.midX,y:reopen.midY)).tap()
        wait { snapshot()["browser_collapsed"] as? Bool == false }
        let settings = rect("nav:settings")
        coordinate(CGPoint(x:settings.midX,y:settings.midY)).tap()
        record("View settings opened by a finger tap")
        coordinate(CGPoint(x:settings.midX,y:settings.midY)).tap()
        // Exercise an ordinary top-toolbar finger button too (theme toggles).
        let zoom = (snapshot()["zoom"] as? NSNumber)?.doubleValue ?? 1
        coordinate(CGPoint(x:278*zoom,y:17*zoom)).tap()
        coordinate(CGPoint(x:278*zoom,y:17*zoom)).tap()
        let face = snapshot()["qa_face"] as? [NSNumber] ?? []
        XCTAssertEqual(face.count,2)
        guard face.count == 2 else { return }
        let at = coordinate(CGPoint(x:face[0].doubleValue,y:face[1].doubleValue))
        let selection = (snapshot()["selection_count"] as? NSNumber)?.intValue
        at.tap()
        RunLoop.current.run(until:Date().addingTimeInterval(0.45))
        XCTAssertEqual((snapshot()["selection_count"] as? NSNumber)?.intValue,selection)
        at.doubleTap()
        wait { ((snapshot()["camera"] as? [String: Any])?["pitch"] as? NSNumber)?.doubleValue ?? 0 > 1.5 }
        XCTAssertEqual((snapshot()["selection_count"] as? NSNumber)?.intValue,selection)
        record("Face aligned overhead by finger double tap without selection")

    }

    func testDXFFromInsertThroughNativeFiles() {
        XCUIDevice.shared.orientation = .landscapeRight
        let app = XCUIApplication(); app.launchArguments = ["--ui-test"]; app.launch()
        let web = app.webViews["cadWebView"]
        func layout() -> [String: Any] {
            let text = (web.value as? String ?? "").components(separatedBy: "\nLayout: ").last ?? "{}"
            guard let data = text.data(using:.utf8), let result = try? JSONSerialization.jsonObject(with:data) as? [String:Any] else { return [:] }
            return result
        }
        func wait(_ condition: @escaping () -> Bool) {
            XCTAssertTrue(XCTWaiter.wait(for:[XCTNSPredicateExpectation(predicate:NSPredicate { _, _ in condition() },object:nil)],timeout:30) == .completed, web.value as? String ?? app.debugDescription)
        }
        func tap(_ key: String) {
            guard let rect = (layout()["rects"] as? [String:Any])?[key] as? [NSNumber], rect.count == 4 else { XCTFail("Missing " + key); return }
            let zoom = (layout()["zoom"] as? NSNumber)?.doubleValue ?? 1
            web.coordinate(withNormalizedOffset:.zero).withOffset(CGVector(dx:(rect[0].doubleValue+rect[2].doubleValue/2)*zoom,dy:(rect[1].doubleValue+rect[3].doubleValue/2)*zoom)).tap()
        }
        wait { (layout()["rects"] as? [String:Any])?["panel:SOLID:INSERT"] != nil }
        tap("panel:SOLID:INSERT")
        wait { (layout()["rects"] as? [String:Any])?["dropdown:sketch.insert_dxf"] != nil }
        let menuShot = XCTAttachment(screenshot:XCUIScreen.main.screenshot()); menuShot.name="Insert DXF in SOLID menu";menuShot.lifetime = .keepAlways;add(menuShot)
        tap("dropdown:sketch.insert_dxf")
        XCTAssertTrue(app.buttons["Cancel"].firstMatch.waitForExistence(timeout:15),app.debugDescription)
        let file = app.cells.matching(NSPredicate(format:"label BEGINSWITH 'SolidCraft-DXF-QA'")).firstMatch
        XCTAssertTrue(file.waitForExistence(timeout:15),app.debugDescription)
        file.tap()
        wait { layout()["active_sketch"] is NSNumber }
        let shot = XCTAttachment(screenshot:XCUIScreen.main.screenshot()); shot.name="DXF imported as editable sketch from Files";shot.lifetime = .keepAlways;add(shot)
    }

    func testLargeCarbodyDXFThroughNativeFiles() {
        XCUIDevice.shared.orientation = .landscapeRight
        let app = XCUIApplication(); app.launchArguments = ["--ui-test"]; app.launch()
        let web = app.webViews["cadWebView"]
        func layout() -> [String: Any] {
            let text = (web.value as? String ?? "").components(separatedBy:"\nLayout: ").last ?? "{}"
            guard let data = text.data(using:.utf8), let result = try? JSONSerialization.jsonObject(with:data) as? [String:Any] else { return [:] }
            return result
        }
        func wait(_ condition: @escaping () -> Bool) {
            XCTAssertTrue(XCTWaiter.wait(for:[XCTNSPredicateExpectation(predicate:NSPredicate { _, _ in condition() },object:nil)],timeout:90) == .completed, "Large DXF import did not finish")
        }
        func tap(_ key: String) {
            guard let r = (layout()["rects"] as? [String:Any])?[key] as? [NSNumber], r.count == 4 else { XCTFail("Missing " + key); return }
            let zoom = (layout()["zoom"] as? NSNumber)?.doubleValue ?? 1
            web.coordinate(withNormalizedOffset:.zero).withOffset(CGVector(dx:(r[0].doubleValue+r[2].doubleValue/2)*zoom,dy:(r[1].doubleValue+r[3].doubleValue/2)*zoom)).tap()
        }
        wait { (layout()["rects"] as? [String:Any])?["panel:SOLID:INSERT"] != nil }
        tap("panel:SOLID:INSERT")
        wait { (layout()["rects"] as? [String:Any])?["dropdown:sketch.insert_dxf"] != nil }
        tap("dropdown:sketch.insert_dxf")
        let file = app.cells.matching(NSPredicate(format:"label BEGINSWITH 'M4 Carbody'")).firstMatch
        XCTAssertTrue(file.waitForExistence(timeout:15),"Test drawing is missing from app Documents")
        file.tap()
        wait { (layout()["imported_curve_count"] as? NSNumber)?.intValue == 38994 }
        XCTAssertEqual((layout()["imported_sketch_count"] as? NSNumber)?.intValue,23)
        // A finger tap still opens the view controls after the large import.
        tap("nav:settings")
        tap("nav:settings")
        let shot = XCTAttachment(screenshot:XCUIScreen.main.screenshot()); shot.name="Large drawing imported with bounded editable sketch parts";shot.lifetime = .keepAlways;add(shot)
    }

    func testNativeFilesPickerAndRestoredHome() {
        XCUIDevice.shared.orientation = .landscapeRight
        let app = XCUIApplication()
        app.launchArguments = ["--ui-test"]
        app.launch()
        let web = app.webViews["cadWebView"]
        let ready = NSPredicate(format: "value CONTAINS 'nav:home'")
        XCTAssertTrue(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: ready, object: web)], timeout: 30) == .completed)
        let text = (web.value as? String ?? "").components(separatedBy: "\nLayout: ").last ?? "{}"
        guard let data = text.data(using: .utf8), let layout = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let values = (layout["rects"] as? [String: Any])?["nav:home"] as? [NSNumber], values.count == 4 else { XCTFail("No Home button"); return }
        let zoom = (layout["zoom"] as? NSNumber)?.doubleValue ?? 1
        func tap(_ x: Double, _ y: Double) { web.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx:x*zoom,dy:y*zoom)).tap() }
        tap(values[0].doubleValue+values[2].doubleValue/2, values[1].doubleValue+values[3].doubleValue/2)
        let homeShot = XCTAttachment(screenshot:XCUIScreen.main.screenshot()); homeShot.name="Home restored"; homeShot.lifetime = .keepAlways; add(homeShot)
        tap(128,17)
        tap(170,76)
        let cancel = app.buttons["Cancel"].firstMatch
        XCTAssertTrue(cancel.waitForExistence(timeout:15), app.debugDescription)
        let shot = XCTAttachment(screenshot:XCUIScreen.main.screenshot()); shot.name="Native Files picker"; shot.lifetime = .keepAlways; add(shot)
        cancel.tap()
    }

    func testParametricEditSaveReopenAndExport() {
        XCUIDevice.shared.orientation = .landscapeRight
        let app = XCUIApplication()
        app.launchArguments = ["--workflow-test"]
        app.launch()
        let web = app.webViews["cadWebView"]
        let exported = NSPredicate(format: "value CONTAINS 'Export saved' AND value CONTAINS 'iPad-Test.json'")
        XCTAssertTrue(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: exported, object: web)], timeout: 90) == .completed, app.debugDescription)
        let result = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        result.name = "Reopened parametric model and exported JSON"
        result.lifetime = .keepAlways
        add(result)
    }

    func testBundledCADStartsAndPersistsStorage() {
        XCUIDevice.shared.orientation = .landscapeRight
        let app = XCUIApplication()
        app.launch()
        let web = app.webViews["cadWebView"]
        let ready = NSPredicate(format: "value CONTAINS 'CAD ready'")
        XCTAssertTrue(XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: ready, object: web)], timeout: 90) == .completed, app.debugDescription)
        func orientationWidget() -> Data? {
            let source = XCUIScreen.main.screenshot().image
            let upright = UIGraphicsImageRenderer(size: source.size).image { _ in
                source.draw(in: CGRect(origin: .zero, size: source.size))
            }
            guard let pixels = upright.cgImage else { return nil }
            let width = CGFloat(pixels.width), height = CGFloat(pixels.height)
            let region = CGRect(x: width * 0.88, y: height * 0.19, width: width * 0.115, height: height * 0.19).integral
            return pixels.cropping(to: region).flatMap { UIImage(cgImage: $0).pngData() }
        }
        let initialOrientation = orientationWidget()
        XCTAssertNotNil(initialOrientation)
        // The first contact of a two-finger gesture must not orbit on landing.
        // XCTest's two-finger-tap API fails hit testing this canvas, so exercise
        // landing here and cover staggered second contact in input-tests.cjs.
        web.coordinate(withNormalizedOffset: CGVector(dx: 0.65, dy: 0.55)).tap()
        XCTAssertEqual(initialOrientation, orientationWidget(), "Finger contact changed the orientation widget")
        let landing = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        landing.name = "First finger landing keeps model orientation"
        landing.lifetime = .keepAlways
        add(landing)
        let hierarchy = XCTAttachment(string: app.debugDescription)
        hierarchy.name = "CAD accessibility hierarchy"
        hierarchy.lifetime = .keepAlways
        add(hierarchy)
        let before = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        before.name = "Sample model before navigation"
        before.lifetime = .keepAlways
        add(before)
        let start = web.coordinate(withNormalizedOffset: CGVector(dx: 0.65, dy: 0.55))
        let end = web.coordinate(withNormalizedOffset: CGVector(dx: 0.79, dy: 0.64))
        start.press(forDuration: 0.1, thenDragTo: end)
        let after = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        after.name = "Sample model after orbit"
        after.lifetime = .keepAlways
        add(after)
        web.pinch(withScale: 1.2, velocity: 0.8)
        let zoom = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        zoom.name = "Two-finger gesture with pan and pinch zoom"
        zoom.lifetime = .keepAlways
        add(zoom)
        let storage = web.value as? String ?? ""
        XCTAssertTrue(storage.contains("read/write OK") || storage.contains("localStorage OK"))
        XCTAssertTrue(storage.contains("\"wasm\":true"))
        XCTAssertTrue(storage.contains("Touch navigation moves:"), "Touch translation did not receive the drag")
        XCTAssertFalse(storage.contains("pan: 0;"), "Two-finger motion did not produce pan input")
        XCTAssertFalse(storage.contains("pinch: 0;"), "Pinch did not produce zoom input")
        XCTAssertTrue(storage.contains("Pencil strokes: 0"), "Finger gestures produced Pencil input")
        let diagnostic = XCTAttachment(string: storage)
        diagnostic.name = "Real WKWebView runtime and storage results"
        diagnostic.lifetime = .keepAlways
        add(diagnostic)
    }
}
