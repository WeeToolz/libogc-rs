"""Checks for the real voice generator's validation and failure boundaries."""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

_spec = importlib.util.spec_from_file_location("generate_voice", Path(__file__).with_name("generate_voice.py"))
gen = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gen)


class VoiceGenerationTests(unittest.TestCase):
    def test_catalog_covers_letters_digits_and_ui(self):
        clips = gen.load_catalog(gen.CATALOG)
        self.assertEqual(len([name for name in clips if len(name) == 1]), 26)
        for name in ["zero", "nine", "good_job", "help", "normal_answer", "backwards_answer", "value_100"]:
            self.assertTrue(clips[name])

    def test_invalid_or_duplicate_names_are_rejected(self):
        for line in ["../outside\tNo.\n", "good_job\tAgain.\n", "empty\t\n"]:
            with tempfile.TemporaryDirectory() as temporary:
                catalog = Path(temporary) / "voice-lines.tsv"
                catalog.write_text(gen.CATALOG.read_text() + line)
                with self.assertRaises(gen.GenerationError):
                    gen.load_catalog(catalog)

    def test_real_tool_failure_does_not_replace_existing_audio(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "a.mp3"
            original = b"previous file must stay unchanged"
            destination.write_bytes(original)
            # Execute a real failing process at the speech boundary.
            tools = {"say": "/usr/bin/false", "ffmpeg": "/usr/bin/false", "ffprobe": "/usr/bin/false"}
            with self.assertRaises(gen.GenerationError):
                gen.generate_clip("a", "Ay.", destination, tools)
            self.assertEqual(destination.read_bytes(), original)
            self.assertEqual(list(Path(temporary).iterdir()), [destination])

    def test_encoder_failure_keeps_previous_file(self):
        say = shutil.which("say")
        if not say:
            self.skipTest("The real Mac speech tool is required for this boundary check")
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "a.mp3"
            original = b"previous completed file"
            destination.write_bytes(original)
            tools = {"say": say, "ffmpeg": "/usr/bin/false", "ffprobe": "/usr/bin/false"}
            with self.assertRaises(gen.GenerationError):
                gen.generate_clip("a", "Ay.", destination, tools)
            self.assertEqual(destination.read_bytes(), original)
            self.assertEqual(list(Path(temporary).iterdir()), [destination])

    def test_actual_silent_mp3_is_rejected(self):
        ffmpeg = shutil.which("ffmpeg")
        ffprobe = shutil.which("ffprobe")
        if not ffmpeg or not ffprobe:
            self.skipTest("FFmpeg and FFprobe are required for the real decode check")
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "silent.mp3"
            gen.run([ffmpeg, "-v", "error", "-f", "lavfi", "-i", "anullsrc=r=22050:cl=mono",
                     "-t", "1", "-codec:a", "libmp3lame", "-b:a", "64k", str(path)])
            with self.assertRaisesRegex(gen.GenerationError, "silent"):
                gen.validate_audio(path, {"ffmpeg": ffmpeg, "ffprobe": ffprobe})


if __name__ == "__main__":
    unittest.main()
