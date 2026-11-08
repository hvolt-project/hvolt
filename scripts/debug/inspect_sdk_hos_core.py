import zipfile
import re

jar_path = '/Applications/DevEco-Studio.app/Contents/plugins/openharmony/lib/sdk-hos-core-26.0.0.461.jar'
with zipfile.ZipFile(jar_path) as z:
    for name in z.namelist():
        if name.endswith('.class'):
            data = z.read(name)
            # Find ascii strings
            strings = re.findall(b'[\x20-\x7e]{4,}', data)
            for s in strings:
                s_str = s.decode('ascii', errors='ignore')
                if any(k in s_str.lower() for k in ['getsdklist', 'sdkmanager', 'hms', 'hos', 'repo.harmonyos', 'dbankcloud', 'supportversion']):
                    print(f"[{name}] {s_str}")
