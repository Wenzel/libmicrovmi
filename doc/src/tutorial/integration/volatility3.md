# volatility3

[volatility3](https://github.com/volatilityfoundation/volatility3) is a framework for extracting digital artifacts and performing forensic investigation
on RAM samples.

Combined with libmicrovmi, you can run volatility3 on top of a live virtual machine's physical memory.

Thanks to volatility3's modular architecture the libmicrovmi integration doesn't require any upstream modification.
Instead we need to indicate to volatility3 how to locate our plugin.

# Setup

This guide assumes you already have a working installation of libmicrovmi Python in a virtualenv.
Please refer to the [documentation](https://wenzel.github.io/libmicrovmi/tutorial/installation.html).

We need the development version of volatility3, from git:

~~~
(venv) $ git clone https://github.com/volatilityfoundation/volatility3
(venv) $ cd volatility3
(venv) $ pip install .
~~~

The `microvmi` python package comes with a [`volatility`](https://github.com/Wenzel/libmicrovmi/tree/master/python/microvmi/volatility)
directory which contains the connection plugin.

We need to add this directory to volatility's search path.

To locate the volatility directory in your `venv`:

~~~
(venv) $ find venv/ -type d -wholename '*microvmi/volatility*'
venv/lib/python3.7/site-packages/microvmi/volatility
~~~

# Usage

## VMI scheme URL

The libmicrovmi handler for volatility is a URL handler with the following syntax:

    vmi://[hypervisor]/vm_name

The hypervisor part is optional. If not specified, it will default to try every builtin driver available.

Additional driver parameters can be specified.

To pass the KVMi socket:

    vmi:///vm_name?kvmi_unix_socket=/tmp/introspector

## Running volatility3

Let's put all of this together and run volatility3 combined with libmicrovmi.

- `-p <plugin_dir>`
- `--single-location vmi://` url

~~~
(venv) $ vol -p <plugin_dir> --single-location vmi:///vm_name <volatility plugin>
~~~

### Example listing processes on Xen

~~~bash
(venv) $ sudo -E ./venv/bin/vol \  # running volatility3 as root (required by the Xen driver)
    -p venv/lib/python3.7/site-packages/microvmi/volatility \  # path to the microvmi connection plugin
    --single-location vmi:///winxp \  # specify the resource location
    windows.pslist.PsList  # volatility's pslist plugin
~~~

🎥 [asciicast](https://asciinema.org/a/6YOXUkEwt53uYcU5rXxoWaFLq)

### Example listing processes on KVM

🎥 [asciicast](https://asciinema.org/a/DTyjM0rnq26RYbFX7hbS7jXvP)
