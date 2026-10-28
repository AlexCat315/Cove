import importlib.util
from pathlib import Path
import shutil
import tempfile
import sys

sys.dont_write_bytecode = True
import unittest

spec = importlib.util.spec_from_file_location('architecture', Path(__file__).with_name('check.py'))
architecture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(architecture)


class CapabilityBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.original = architecture.ROOT
        self.directory = tempfile.TemporaryDirectory()
        architecture.ROOT = Path(self.directory.name).resolve()
        for package in architecture.PACKAGES:
            target = architecture.ROOT / package / 'Package.swift'
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(self.original / package / 'Package.swift', target)

    def tearDown(self):
        architecture.ROOT = self.original
        self.directory.cleanup()

    def test_manifest_graph_has_no_duplicate_owners(self):
        self.assertFalse(architecture.check()[0])
        manifest = architecture.ROOT / 'Runtime/Package.swift'
        manifest.write_text(manifest.read_text() + '\n.target(name: "CFreeType")')
        self.assertTrue(any('declared more than once' in error for error in architecture.check()[0]))

    def test_public_layer_dependency_cannot_hide_an_upward_import(self):
        source = architecture.ROOT / 'Compose/Sources/CoveUICompose/Invalid.swift'
        source.parent.mkdir(parents=True)
        source.write_text('@testable import class CoveUIRuntime.PlatformEventBridge\n')
        self.assertTrue(any('forbidden import CoveUIRuntime' in error for error in architecture.check()[0]))

    def test_lower_capability_bridge_is_not_a_public_contract(self):
        source = architecture.ROOT / 'Runtime/Sources/CoveUIRuntime/Invalid.swift'
        source.parent.mkdir(parents=True)
        source.write_text('internal import CFreeType\n')
        self.assertTrue(any("another capability's bridge" in error for error in architecture.check()[0]))

    def test_internal_product_cannot_return(self):
        manifest = architecture.ROOT / 'Graphics/Package.swift'
        manifest.write_text(manifest.read_text() + '\n.library(name: "NativeRHI", targets: ["NativeRHI"])')
        self.assertTrue(any('implementation module exported' in error for error in architecture.check()[0]))


if __name__ == '__main__':
    unittest.main()
