import WidgetKit

// Exposed to Rust via C ABI. Reloads this app's WidgetKit timelines so the
// widget re-reads the App Group snapshot after the app writes it.
@_cdecl("notefix_reload_widgets")
public func notefix_reload_widgets() {
    if #available(macOS 11.0, *) {
        WidgetCenter.shared.reloadAllTimelines()
    }
}
