`Mesa <https://mesa3d.org>`_ - The 3D Graphics Library
======================================================

Build and install for UWP
------
Use a meson fork that supports UWP.

.. code-block:: sh

  $ python meson.py setup build_release --wipe --backend=vs --uwp --buildtype=release -Dcpp_std=vc++17 -Dcpp_args=["'/D _XBOX_UWP'"] -Dc_args=["'/D _XBOX_UWP'"] -Db_pch=false -Dc_winlibs=[] -Dcpp_winlibs=[]
  $ python meson.py install -C build_release

Original Source
------

This repository lives at https://gitlab.freedesktop.org/mesa/mesa.
Other repositories are likely forks, and code found there is not supported.
