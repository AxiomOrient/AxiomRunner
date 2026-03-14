import unittest
import app


class AppTest(unittest.TestCase):
    def test_ping(self):
        self.assertEqual(app.ping(), "ok")


if __name__ == "__main__":
    unittest.main()
