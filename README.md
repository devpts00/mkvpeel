# mkvpeel

## Summary

Tool to automatically peel extra audio and subtitle tracks from `mkv` files.

## Description

Nowadays people tend to pack mkv files with dozens of audio and subtitle tracks of many 
languages and/or various translations. Obviously, it makes mkv usable for broad range of watchers 
to pick up track with favorite translator and/or just compatible with the available hardware.
However, if making personal movie collections on a well known hardware assembly, there is a constant
need to remove extra tracks one will never use to save space. This tool allows to specify the rules 
and peel off unneeded tracks. The tool monitors the specified directory and runs `mkvmerge`.
The resulting mkv are placed into the other directory. The tool is rather lightweight and can 
be run on NAS in docker.