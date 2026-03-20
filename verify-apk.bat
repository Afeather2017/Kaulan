@echo off
set "JAVA_HOME=C:\Program Files\Android\Android Studio\jbr"
C:\Users\Administrator\AppData\Local\Android\Sdk\build-tools\35.0.0\apksigner verify --verbose %1
