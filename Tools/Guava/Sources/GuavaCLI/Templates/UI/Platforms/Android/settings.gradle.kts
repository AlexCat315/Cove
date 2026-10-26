import groovy.json.JsonSlurper

pluginManagement {
    val androidPluginVersion = "8.11.1"
    repositories { google(); mavenCentral(); gradlePluginPortal() }
    plugins {
        id("com.android.application") version androidPluginVersion
        id("com.android.library") version androidPluginVersion
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "GuavaApplication"
val projectRoot = rootDir.resolve("../..").canonicalFile
val manifest = JsonSlurper().parse(projectRoot.resolve("guava.json")) as Map<*, *>
val toolchain = manifest["toolchain"] as Map<*, *>
val version = toolchain["sdkVersion"] as String
val environment = System.getenv()
val os = System.getProperty("os.name")
val home = File(System.getProperty("user.home"))
val installation = environment["GUAVA_HOME"]?.let(::File) ?: when {
    os.startsWith("Windows") -> File(environment["LOCALAPPDATA"], "Guava")
    os.startsWith("Linux") -> File(environment["XDG_DATA_HOME"] ?: home.resolve(".local/share").path, "guava")
    else -> home.resolve(".guava")
}
val selection = projectRoot.resolve(".guava/sdk.json")
val semver = Comparator<File> { a, b ->
    fun parts(f: File) = f.name.split(".").mapNotNull { it.toIntOrNull() }
    val pa = parts(a)
    val pb = parts(b)
    pa.zip(pb).firstNotNullOfOrNull { (x, y) -> x.compareTo(y).takeIf { it != 0 } }
        ?: pa.size.compareTo(pb.size)
}
val newestInstalled = installation.resolve("sdks").listFiles()
    ?.filter { it.resolve("Package.swift").isFile }
    ?.maxWithOrNull(semver)
val sdk = environment["GUAVA_SDK_ROOT"]?.let(::File)
    ?: if (selection.isFile) File((JsonSlurper().parse(selection) as Map<*, *>)["path"] as String)
       else if (version == "latest") newestInstalled
       else installation.resolve("sdks/$version")
check(sdk?.resolve("Package.swift")?.isFile == true) { "Install the SDK with guava sdk install $version" }
requireNotNull(sdk)
gradle.beforeProject { extensions.extraProperties.set("guavaSDK", sdk) }
include(":app", ":guavaRuntime")
project(":guavaRuntime").projectDir = sdk.resolve("Hosts/Android/Runtime")
