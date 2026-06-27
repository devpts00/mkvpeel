# mkvpeel

## Summary

Tool to automatically peel extra audio and subtitle tracks from `mkv` files.

## Description

Nowadays people tend to pack mkv files with dozens of audio and subtitle tracks of many 
languages and/or various translations. Obviously, it makes the mkv usable for a broad range of watchers 
to pick up a track with their favorite translator and/or just compatible with the available hardware.
However, when making personal movie collections on the well known hardware assembly, there is a constant
need to remove extra tracks one will never use to save space. This tool allows to specify the rules 
and peel off unneeded tracks. The tool monitors the specified directory and runs `mkvmerge` with 
corresponding arguments. The resulting mkv files are placed into the other directory. 
The tool is lightweight (~3.5Mb RSS) enough to be run on NAS in docker. 