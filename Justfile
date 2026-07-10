set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

import 'just/bench.just'
import 'just/fixtures.just'
import 'just/verify.just'
import 'just/web.just'

default:
    @just --list
