## Quickget configurations

This repository releases daily (0:00 UTC) JSON-formatted data containing everything necessary to download and create VMs for many popular operating systems.

## Format

Each entry is formatted as follows

```json
{
    "os": "os_name",
    "pretty_name": "OS Name",
    "homepage": "https://os.homepage", // OPTIONAL
    "description": "A description of the OS", // OPTIONAL
    "releases" [ release ]
}
```

Releases contain configurations used. They are formatted as follows

```json
{
    "release": "release_name", // OPTIONAL
    "edition": "edition_name", // OPTIONAL
    "guest_os": "guest_os", // SKIPPED IF == Linux
    "arch": "arch", // SKIPPED IF == x86_64
    "iso": [ Source ],
    "img": [ Source ],
    "fixed_iso": [ Source ],
    "floppy": [ Source ],
    "disk_images": [ Disk ],
}
```

Disks contain the following
```json
"source": Source,
"size": 1234, // OPTIONAL, IN BYTES
"format": "format", // OPTIONAL
```

Sources are tagged unions which can contain the following

```json
"web": WebSource,
"file_name": String,
"custom"
```

WebSource is formatted as follows

```json
{
    "url": "https://source.url",
    "checksum": "checksum", // OPTIONAL
    "archive_format": "archive_format", // OPTIONAL
    "file_name": "file_name", // OPTIONAL
}
```
