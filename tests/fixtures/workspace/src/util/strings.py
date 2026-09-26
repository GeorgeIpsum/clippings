def normalise(text):
    # TODO normalise unicode before comparing
    return text.strip()  # BUG(bob) drops leading tabs too
