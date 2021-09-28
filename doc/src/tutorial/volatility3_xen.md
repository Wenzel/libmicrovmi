# Listing Windows 10 Processes using Volatility3 on Xen

In this tutorial, we will use the [volatility3](https://github.com/volatilityfoundation/volatility3) memory forensic framework to list the processes
on a running Windows 10 VM on Xen hypervisor.

Thanks to volatility3's modular architecture the libmicrovmi integration doesn't require any upstream modification.
Instead we need to indicate to volatility3 how to locate our plugin.

## Requirements

- [libmicrovmi Python bindings](./installation.md) in a virtualenv `venv`
- Xen >= 4.11
- Windows 10 VM running

## 1 - Install volatility3

We need the latest development version of volatility3, from git:

~~~
(venv) $ git clone https://github.com/volatilityfoundation/volatility3
(venv) $ cd volatility3
(venv) $ pip install .
~~~

## 2 - Locate microvmi volatility plugin directory

The `microvmi` python package comes with a [`volatility`](https://github.com/Wenzel/libmicrovmi/tree/master/python/microvmi/volatility) directory which contains the connection plugin.

We need to add this directory to **volatility's search path**.

To locate the volatility directory in your `venv`:

~~~
(venv) $ find venv/ -type d -wholename '*microvmi/volatility*'
venv/lib/python3.7/site-packages/microvmi/volatility
~~~

## 3 - Running volatility with microvmi plugin

The Microvmi volatility plugin recognizes the `vmi://` URL scheme.

For Xen we need to pass the `vm_name` parameter.
Assuming that our Xen domain is named `win10`:

~~~bash
(venv) $ sudo -E ./venv/bin/vol \  # running volatility3 as root (required by the Xen driver)
    --plugin-dirs venv/lib/python3.7/site-packages/microvmi/volatility \  # path to the microvmi connection plugin
    -vvv \ # verbosity
    --single-location 'vmi:///?vm_name=win10' \  # specify the resource location
    windows.pslist.PsList  # volatility's pslist plugin
~~~

⚠️ To debug libmicrovmi initialization: `export RUST_LOG=debug`

For a complete overview of the URL parameters, check the [documentation](./../reference/integration/volatility3.md)
