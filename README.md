# rsort

## Python Import Organizer

A high-performance command-line tool written in Rust that automatically organizes and sorts Python import statements.

## Features

- Organizes imports into logical groups:
  - Future imports (`__future__`)
  - Standard library imports
  - Third-party library imports
  - Local library imports
- Maintains proper spacing between import blocks
- Processes single files or entire directories recursively
- Parallel processing support for better performance
- Automatic backup creation before modifications
- Preserves existing code formatting outside of import blocks

## Installation

### Build from source
```bash
cargo build --release
```

### The binary will be available in target/release/

## Usage

```bash
rsort path/to/python/file/or/directory
```

### Specify number of threads for parallel processing
```bash
rsort path/to/python/file/or/directory -t 4
```

## Import Grouping Rules

Imports are organized into the following groups, with blank lines between each group:

1. `__future__` imports
2. Standard library imports
3. Third-party library imports
4. Local library imports (starting with `.` or `..`)

Within each group:
- `import` statements come before `from ... import` statements
- Imports are sorted alphabetically (case-insensitive)

## Safety Features

- Creates automatic backups in the system's temp directory before modifying files
- Only modifies files when changes are necessary
- Preserves all non-import code exactly as is

## Command Line Options

```bash
rsort path/to/python/file/or/directory -t 4
```

```bash
Options:
  -t, --threads <THREADS>    Number of threads for parallel processing [default: 4]
  -h, --help                Display help information
  -V, --version             Display version information
```

## Example

Input:
```python
import random
from datetime import datetime
import os
from . import local_module
import django
from __future__ import annotations
```

Output:
```python
from __future__ import annotations

import os
import random
from datetime import datetime

import django

from . import local_module
```

## License

[MIT](LICENSE)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
