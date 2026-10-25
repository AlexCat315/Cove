import GuavaAndroidContract
import Foundation
import NativeRHI
import GuavaUIRuntime
import GuavaUIDevTools
import CAndroidHost

/// Activity-retained state. Surface loss only releases the swapchain.
@MainActor
final class AndroidApplication {
    let device: Device
    let renderer: NativeDrawListRenderer
    let scene: HostedScene
    private(set) var scale: Float
    let runtime: RuntimeOptions

    func updateScale(_ value: Float) {
        let next = value.isFinite && value > 0 ? value : 1
        guard scale != next else { return }
        scale = next
        scene.invalidate()
    }

    init(scale: Float, checkpoint: String, launch: SceneLaunch, runtime: RuntimeOptions) throws {
        self.runtime = runtime
        self.scale = scale.isFinite && scale > 0 ? scale : 1
        device = try Device.make(DeviceConfig(enableValidation: false))
        renderer = try NativeDrawListRenderer(device: device)
        try renderer.configure(format: .rgba8Unorm)
        scene = try HostedScene(checkpoint: checkpoint, launch: launch, scale: self.scale, device: device)
        if let launch = runtime.devtools {
            var configuration = DevToolsConfig(port: 0, appTitle: launch.appTitle)
            configuration.launch = launch
            try scene.content.configureDiagnostics(configuration) { [weak self] in
                if let self { guava_android_schedule_diagnostics(Unmanaged.passUnretained(self).toOpaque()) }
            }
        }
    }
}
