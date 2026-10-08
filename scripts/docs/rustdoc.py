# SPDX-FileCopyrightText: 2026 roolrz
# SPDX-License-Identifier: Apache-2.0

"""Restore upstream links left unresolved in inherited Rustdoc descriptions."""

from html import escape
from html.parser import HTMLParser

# Rustdoc 1.97 leaves this shorthand unresolved in inherited Iterator::cmp
# descriptions. The standard-library trait remains the owner of that section.
UPSTREAM_LINKS = {
    'Ord#lexicographical-comparison':
        'https://doc.rust-lang.org/core/cmp/trait.Ord.html#lexicographical-comparison',
}


class UpstreamLinks(HTMLParser):
    def __init__(self, text):
        super().__init__(convert_charrefs=True)
        self.offsets = [0]
        for line in text.split('\n')[:-1]:
            self.offsets.append(self.offsets[-1] + len(line) + 1)
        self.replacements = []
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        if tag != 'a' or dict(attrs).get('href') not in UPSTREAM_LINKS:
            return
        attributes = []
        for name, value in attrs:
            if name == 'href':
                value = UPSTREAM_LINKS[value]
            attributes.append(name if value is None else f'{name}="{escape(value, quote=True)}"')
        line, column = self.getpos()
        start = self.offsets[line - 1] + column
        self.replacements.append((start, len(self.get_starttag_text()),
                                  '<a ' + ' '.join(attributes) + '>'))


def fix_upstream_links(text):
    # Avoid parsing most pages. Only actual anchor attributes are rewritten;
    # prose, escaped source examples, scripts and unrelated URLs retain bytes.
    if not any(link in text for link in UPSTREAM_LINKS):
        return text
    for start, length, replacement in reversed(UpstreamLinks(text).replacements):
        text = text[:start] + replacement + text[start + length:]
    return text
