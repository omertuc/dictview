#!/usr/bin/env python3

from rich import print
import json


class GST:
    WKEY="WS"
    def __init__(self, words):
        self.tree = {self.WKEY: []}
        self.add_words(words)

    def add_words(self, words):
        for word in words:
            for suffix_length in range(len(word)):
                suffix = word[-suffix_length - 1 :]

                cur = self.tree
                for c in suffix:
                    if c not in cur:
                        cur[c] = {self.WKEY: {word}}
                    cur = cur[c]

                    if self.WKEY not in cur:
                        cur[self.WKEY] = set()
                    cur[self.WKEY].add(word)

    def search(self, word_to_find):
        cur = self.tree
        for c in word_to_find:
            if c not in cur:
                return []

            cur = cur[c]

        return list(sorted("".join(word) for word in cur[self.WKEY]))


def main():
    with open("/home/omer/testwords/jdata.json") as data:
        tree = GST([x[4] for x in json.load(data)])

    while True:
        founds = tree.search(input("Enter a word: ").strip())
        print(", ".join("".join(reversed(word)) for word in founds))


if __name__ == "__main__":
    main()
