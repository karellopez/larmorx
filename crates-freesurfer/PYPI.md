# larmorx-freesurfer: FreeSurfer's programs for larmorx (FreeSurfer Software License)

larmorx-freesurfer is a companion of [larmorx](https://pypi.org/project/larmorx/), the
neuroimaging toolbox reimplemented in Rust. It holds **bit-exact replicas** of **FreeSurfer
8.2.0's programs**: Rust translations of FreeSurfer's source code, written to give FreeSurfer's
output bit for bit, with every option, message, exit status and output file of the original
programs. It is being built; its programs come one after the other.

## Licence

larmorx-freesurfer is distributed under the
[FreeSurfer Software License](https://github.com/karellopez/larmorx/blob/main/crates-freesurfer/LICENSE)
(version 1.0, February 2011), as a modified version of FreeSurfer: research use only, not reviewed
or approved by the Food and Drug Administration or any other agency, clinical applications neither
recommended nor advised, provided as is. It does not present itself as FreeSurfer and is not
endorsed by The General Hospital Corporation or FreeSurfer's developers. It needs no FreeSurfer
licence key.

## How larmorx uses it

larmorx-freesurfer is a separate program, `larmorx-freesurfer`; larmorx (Apache-2.0) runs it as
a separate process and exchanges files with it only. The program also runs directly, with the
original program's arguments after the family:

```bash
larmorx-freesurfer --help
```
