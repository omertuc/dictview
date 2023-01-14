from rich import print
import time
import json


def add_suffix(tree, suffix, word):
    cur = tree
    for c in suffix:
        if "words" not in cur:
            cur["words"] = {word}
        else:
            cur["words"].add(word)

        if c not in cur:
            cur[c] = {}

        cur = cur[c]


def add_word(tree, word):
    for suffix_length in range(len(word)):
        add_suffix(tree, word[-suffix_length - 1 :], word)


def construct(words):
    tree = {}
    for word in words:
        add_word(tree, word)

    return tree


def search(tree_to_search, word_to_find):
    cur = tree_to_search
    words = set()
    for c in word_to_find:
        if c not in cur:
            return []

        cur = cur[c]
        words |= cur.get("words", set())

    return list(sorted((word for word in words if word_to_find in word)))


def load_tree():
    with open("/home/omer/testwords/jdata.json") as data:
        all_data = json.load(data)
    words = [x[4] for x in all_data]

    x = time.time()
    tree = construct(words)
    y = time.time()

    print(f"Tree took {(y - x) * 1000:.2f}ms")

    return words, tree


def search_naive(words):
    x = time.time()
    founds = []
    for word in words:
        if "להיות" in word:
            founds.append(word)
    y = time.time()

    print(f"Naive took {(y - x) * 1000:.2f}ms")

    return founds, x, y


def search_smart(tree):
    x = time.time()
    founds = search(tree, "להיות")
    y = time.time()

    print(f"Quick took {(y - x) * 1000:.2f}ms")

    return founds, x, y


def main():
    words, tree = load_tree()
    founds, x, y = search_naive(words)
    founds2, x2, y2 = search_smart(tree)

    print(f"Quick is {(y - x) / (y2 - x2):.2f} times faster!")

    print(founds, founds2)
    assert list(sorted(founds)) == founds2


if __name__ == "__main__":
    main()
