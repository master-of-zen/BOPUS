#!/usr/bin/env python3

# Import the AudioSegment class for processing audio and the
# split_on_silence function for separating out silent chunks.
from pydub import AudioSegment
from pydub.silence import split_on_silence
import matplotlib.pyplot as plt
import numpy as np

sound = AudioSegment.from_file('aud.mkv', codec='libopus')


print(sound)
chunks = split_on_silence(sound, min_silence_len=200, silence_thresh=-16)

print(chunks)
# samples = np.array(sound.get_array_of_samples())), 

# x = np.arange(0, len(samples))
# plt.show()

