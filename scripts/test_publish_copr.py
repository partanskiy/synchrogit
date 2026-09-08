#!/usr/bin/env python3
import copy
import io
import unittest
from unittest.mock import Mock
import urllib.error

from publish_copr import request_build, should_publish


class CoprPublication(unittest.TestCase):
    def setUp(self):
        self.repo = "owner/project"
        self.release = {"tag_name": "v26.9.1", "draft": False, "prerelease": False}
        self.event = {"workflow_run": {"conclusion": "success", "event": "push",
                      "head_branch": "v26.9.1", "head_sha": "release-sha",
                      "head_repository": {"full_name": self.repo}}}

    def decide(self, event=None, release=None, name="workflow_run"):
        return should_publish(name, event or self.event, self.repo, release or self.release, "release-sha")

    def test_only_the_completed_latest_release_can_trigger(self):
        self.assertTrue(self.decide())
        self.assertTrue(self.decide(name="workflow_dispatch"))
        self.assertFalse(self.decide(name="release"))  # Asset/notes edits never rebuild.
        for key, value in [("conclusion", "failure"), ("event", "pull_request"),
                           ("head_branch", "v26.9.0"), ("head_sha", "old-release-sha"),
                           ("head_repository", {"full_name": "fork/project"})]:
            event = copy.deepcopy(self.event)
            event["workflow_run"][key] = value
            self.assertFalse(self.decide(event=event), key)
        for key, value in [("draft", True), ("prerelease", True), ("tag_name", "v26.9.1-rc.1")]:
            release = dict(self.release, **{key: value})
            self.assertFalse(self.decide(release=release), key)

    def test_post_once_without_retrying_or_exposing_the_secret_url(self):
        url = "https://copr.fedorainfracloud.org/webhooks/custom/test-only-placeholder/"
        opener = Mock(return_value=io.BytesIO(b"ok"))
        request_build(url, "v26.9.1", opener)
        self.assertEqual(opener.call_count, 1)
        self.assertEqual(opener.call_args.args[0].method, "POST")
        opener = Mock(side_effect=urllib.error.URLError(url))
        with self.assertRaises(RuntimeError) as error:
            request_build(url, "v26.9.1", opener)
        self.assertNotIn(url, str(error.exception))
        self.assertEqual(opener.call_count, 1)

    def test_wrong_hosts_and_plaintext_urls_never_receive_a_request(self):
        opener = Mock()
        for url in ["", "http://copr.fedorainfracloud.org/webhooks/custom/key/",
                    "https://example.com/webhooks/custom/key/",
                    "https://copr.fedorainfracloud.org/webhooks/github/key/",
                    "https://user:pass@copr.fedorainfracloud.org/webhooks/custom/key/"]:
            with self.assertRaises(ValueError):
                request_build(url, "v26.9.1", opener)
        opener.assert_not_called()


if __name__ == "__main__":
    unittest.main()
