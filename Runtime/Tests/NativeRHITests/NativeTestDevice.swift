import XCTest

@testable import NativeRHI

/// Creates a device for contract tests, skipping when this host cannot provide
/// the requested native backend. CI runners without a Vulkan driver or WSI
/// extensions must not fail the contract suites.
func makeTestDevice(_ api: GraphicsAPI, enableValidation: Bool = false) throws -> Device {
    guard NativeRHI.isCompiledIn(api) else { throw XCTSkip("Backend is not compiled") }
    #if (os(Windows) || os(Linux) || os(Android)) && canImport(CVulkanHeaders)
    if api == .vulkan, !VulkanBackend.isAvailable { throw XCTSkip("No Vulkan loader / ICD") }
    #endif
    return try Device.make(DeviceConfig(preferredBackends: [api], enableValidation: enableValidation))
}
