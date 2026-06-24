// Global State
let connectionPollInterval = null;
let deviceUpdateInterval = null;
let midiStatusInterval = null;
let currentTab = 'lights';

let lightsList = [];
let groupsList = [];
let dashboardLayout = []; // Array of {type: 'light'|'group', id: 'X'}
let activeMappings = {};

// Drag and drop tracking
let draggedIndex = null;

// Wait for pywebview API to be ready
window.addEventListener('pywebviewready', () => {
    console.log("pywebview ready.");
    initApp();
});

function initApp() {
    // 1. Register global MIDI callback for backend pushing
    window.onMidiActivity = (eventKey, value, learnCache) => {
        if (eventKey === null) {
            // Signal to refresh MIDI status
            refreshMidiStatus();
        } else {
            updateMidiTerminal(eventKey, value, learnCache);
        }
    };

    // 2. Start connection polling loop
    startConnectionPolling();
}

/* Onboarding & Connection Handling */
function startConnectionPolling() {
    if (connectionPollInterval) clearInterval(connectionPollInterval);
    
    pollConnection();
    connectionPollInterval = setInterval(pollConnection, 1000);
}

function pollConnection() {
    if (!window.pywebview || !window.pywebview.api) return;

    window.pywebview.api.get_connection_status().then(status => {
        updateOnboardingUI(status);
        
        if (status === 'connected') {
            clearInterval(connectionPollInterval);
            connectionPollInterval = null;
            
            // Enter dashboard mode
            document.getElementById('onboarding-overlay').classList.add('hidden');
            document.getElementById('app-container').classList.remove('hidden');
            
            // Load dashboard data
            loadDashboardData();
            
            // Periodically refresh device states (every 5 seconds)
            if (deviceUpdateInterval) clearInterval(deviceUpdateInterval);
            deviceUpdateInterval = setInterval(refreshDevicesSilent, 5000);
        }
    }).catch(err => {
        console.error("Error polling connection state:", err);
    });
}

function updateOnboardingUI(status) {
    // Hide all states first
    document.getElementById('state-searching').classList.add('hidden');
    document.getElementById('state-link-needed').classList.add('hidden');
    document.getElementById('state-manual-ip').classList.add('hidden');
    document.getElementById('state-error').classList.add('hidden');

    if (status === 'searching') {
        document.getElementById('state-searching').classList.remove('hidden');
    } else if (status === 'needs_link') {
        document.getElementById('state-link-needed').classList.remove('hidden');
        window.pywebview.api.get_bridge_ip().then(ip => {
            document.getElementById('discovered-ip').innerText = ip || "Unknown";
        });
    } else if (status === 'error') {
        document.getElementById('state-error').classList.remove('hidden');
    } else if (status === 'idle') {
        document.getElementById('state-manual-ip').classList.remove('hidden');
    }
}

function showManualIpForm() {
    if (connectionPollInterval) clearInterval(connectionPollInterval);
    updateOnboardingUI('idle');
}

function submitManualIp() {
    const ip = document.getElementById('manual-ip-input').value.trim();
    if (!ip) {
        alert("Please enter a valid IP address.");
        return;
    }
    
    updateOnboardingUI('searching');
    window.pywebview.api.connect_bridge(ip).then(() => {
        startConnectionPolling();
    });
}

function restartDiscovery() {
    updateOnboardingUI('searching');
    window.pywebview.api.connect_bridge("").then(() => {
        startConnectionPolling();
    });
}

function disconnectBridge() {
    if (confirm("Disconnect from Hue Bridge and log out?")) {
        if (deviceUpdateInterval) clearInterval(deviceUpdateInterval);
        if (midiStatusInterval) clearInterval(midiStatusInterval);
        
        document.getElementById('app-container').classList.add('hidden');
        document.getElementById('onboarding-overlay').classList.remove('hidden');
        showManualIpForm();
        window.pywebview.api.connect_bridge("disconnect");
    }
}

/* Tab Navigation */
function switchTab(tabName) {
    currentTab = tabName;
    document.getElementById('tab-btn-lights').classList.toggle('active', tabName === 'lights');
    document.getElementById('tab-btn-midi').classList.toggle('active', tabName === 'midi');
    
    document.getElementById('tab-content-lights').classList.toggle('active', tabName === 'lights');
    document.getElementById('tab-content-midi').classList.toggle('active', tabName === 'midi');
    
    if (tabName === 'midi') {
        refreshMidiStatus();
        if (midiStatusInterval) clearInterval(midiStatusInterval);
        midiStatusInterval = setInterval(refreshMidiStatus, 3000);
    } else {
        if (midiStatusInterval) {
            clearInterval(midiStatusInterval);
            midiStatusInterval = null;
        }
    }
}

/* Dashboard Loading & Refreshing */
function loadDashboardData() {
    // 1. Get Bridge IP for Header
    window.pywebview.api.get_bridge_ip().then(ip => {
        document.getElementById('header-ip').innerText = ip;
    });

    // 2. Fetch layout and devices
    window.pywebview.api.get_dashboard_layout().then(layout => {
        dashboardLayout = layout || [];
        refreshDevices();
    });

    // 3. Populate MIDI configuration
    populateMidiConfig();
}

function refreshDevices() {
    if (!window.pywebview || !window.pywebview.api) return;
    
    window.pywebview.api.get_lights_and_groups().then(data => {
        lightsList = data.lights || [];
        groupsList = data.groups || [];
        renderDashboardWidgets();
    });
}

function refreshDevicesSilent() {
    if (!window.pywebview || !window.pywebview.api) return;
    
    window.pywebview.api.get_lights_and_groups().then(data => {
        lightsList = data.lights || [];
        groupsList = data.groups || [];
        updateDevicesUIValues();
    });
}

/* Render Configurable Dashboard Widgets */
function renderDashboardWidgets() {
    const container = document.getElementById('dashboard-widgets-container');
    const placeholder = document.getElementById('dashboard-empty-placeholder');
    
    if (dashboardLayout.length === 0) {
        container.innerHTML = '';
        placeholder.classList.remove('hidden');
        return;
    }
    
    placeholder.classList.add('hidden');
    container.innerHTML = '';
    
    dashboardLayout.forEach((item, index) => {
        // Find corresponding device object
        let device = null;
        if (item.type === 'light') {
            device = lightsList.find(l => l.id == item.id);
        } else {
            device = groupsList.find(g => g.id == item.id);
        }
        
        if (!device) {
            // Widget not found (deleted from bridge), render missing item placeholder
            device = { id: item.id, name: `Missing ${item.type} (${item.id})`, on: false, bri: 0, hue: 0, sat: 0, missing: true };
        }
        
        const cardHTML = createDeviceCard(device, item.type, index);
        container.insertAdjacentHTML('beforeend', cardHTML);
    });
    
    setupDragAndDropEvents();
}

function createDeviceCard(device, type, index) {
    const isOn = device.on;
    const hexColor = hueSatToHex(device.hue, device.sat);
    const isMissing = device.missing;
    
    return `
        <div id="card-${index}" class="device-card ${isOn ? 'on' : ''}" draggable="true" data-index="${index}">
            <div class="device-info-row">
                <div class="device-info-left">
                    <span class="drag-handle" title="Drag to reorder">⠿</span>
                    <span class="device-name">${device.name}</span>
                </div>
                <div>
                    ${!isMissing ? `
                    <label class="switch">
                        <input type="checkbox" id="${type}-${device.id}-toggle" ${isOn ? 'checked' : ''} 
                               onchange="onDeviceToggle('${type}', '${device.id}', this.checked)">
                        <span class="slider-toggle"></span>
                    </label>
                    ` : '<span style="color:var(--text-muted);font-size:0.8rem;">offline</span>'}
                    <button class="btn-remove-widget" onclick="removeWidget(${index})" title="Remove Widget">&times;</button>
                </div>
            </div>
            
            ${!isMissing ? `
            <div class="device-controls-row">
                <div class="slider-control">
                    <label>Bri</label>
                    <input type="range" id="${type}-${device.id}-bri" min="0" max="254" value="${device.bri}" 
                           oninput="onDeviceBrightnessInput('${type}', '${device.id}', this.value)">
                </div>
                <div class="color-picker-control">
                    <div class="color-picker-wrapper" style="background-color: ${hexColor}" id="${type}-${device.id}-color-preview">
                        <input type="color" id="${type}-${device.id}-color" value="${hexColor}" 
                               oninput="onDeviceColorInput('${type}', '${device.id}', this.value)">
                    </div>
                </div>
            </div>
            ` : ''}
        </div>
    `;
}

function updateDevicesUIValues() {
    dashboardLayout.forEach((item) => {
        let device = null;
        if (item.type === 'light') {
            device = lightsList.find(l => l.id == item.id);
        } else {
            device = groupsList.find(g => g.id == item.id);
        }
        
        if (!device) return;
        
        const index = dashboardLayout.findIndex(x => x.type === item.type && x.id == item.id);
        if (index === -1) return;
        
        const card = document.getElementById(`card-${index}`);
        const toggle = document.getElementById(`${item.type}-${device.id}-toggle`);
        const briSlider = document.getElementById(`${item.type}-${device.id}-bri`);
        const colorInput = document.getElementById(`${item.type}-${device.id}-color`);
        const colorPreview = document.getElementById(`${item.type}-${device.id}-color-preview`);

        if (card) {
            card.classList.toggle('on', device.on);
        }
        if (toggle && document.activeElement !== toggle) {
            toggle.checked = device.on;
        }
        if (briSlider && document.activeElement !== briSlider) {
            briSlider.value = device.bri;
        }
        if (colorInput && document.activeElement !== colorInput) {
            const hexColor = hueSatToHex(device.hue, device.sat);
            colorInput.value = hexColor;
            if (colorPreview) {
                colorPreview.style.backgroundColor = hexColor;
            }
        }
    });
}

/* Drag and Drop Layout Reordering */
function setupDragAndDropEvents() {
    const cards = document.querySelectorAll('.device-widgets-grid .device-card, #dashboard-widgets-container .device-card');
    
    cards.forEach(card => {
        card.addEventListener('dragstart', (e) => {
            draggedIndex = parseInt(card.getAttribute('data-index'));
            card.classList.add('dragging');
            e.dataTransfer.effectAllowed = 'move';
        });
        
        card.addEventListener('dragend', () => {
            card.classList.remove('dragging');
            cards.forEach(c => c.classList.remove('drag-over'));
        });
        
        card.addEventListener('dragover', (e) => {
            e.preventDefault();
            card.classList.add('drag-over');
        });
        
        card.addEventListener('dragleave', () => {
            card.classList.remove('drag-over');
        });
        
        card.addEventListener('drop', (e) => {
            e.preventDefault();
            const targetIndex = parseInt(card.getAttribute('data-index'));
            
            if (draggedIndex !== null && draggedIndex !== targetIndex) {
                // Reorder dashboardLayout array
                const movedItem = dashboardLayout.splice(draggedIndex, 1)[0];
                dashboardLayout.splice(targetIndex, 0, movedItem);
                
                // Save layout to config
                window.pywebview.api.save_dashboard_layout(dashboardLayout).then(() => {
                    renderDashboardWidgets();
                });
            }
            draggedIndex = null;
        });
    });
}

/* Widget Modal Management (Checklist / Multiselect) */
function openAddWidgetsModal() {
    document.getElementById('modal-search-input').value = '';
    document.getElementById('add-widgets-modal').classList.remove('hidden');
    
    // Fetch fresh lists of devices
    window.pywebview.api.get_lights_and_groups().then(data => {
        lightsList = data.lights || [];
        groupsList = data.groups || [];
        populateModalLists();
    });
}

function closeAddWidgetsModal() {
    document.getElementById('add-widgets-modal').classList.add('hidden');
}

function populateModalLists() {
    const groupsContainer = document.getElementById('modal-groups-list');
    const lightsContainer = document.getElementById('modal-lights-list');
    
    // Helper to check if item is already on dashboard
    const isAdded = (type, id) => {
        return dashboardLayout.some(x => x.type === type && x.id == id);
    };

    // Populate Groups Checklist
    if (groupsList.length === 0) {
        groupsContainer.innerHTML = '<span style="color:var(--text-muted); font-size: 0.85rem; padding: 4px;">No groups found</span>';
    } else {
        groupsContainer.innerHTML = groupsList.map(g => {
            const addedClass = isAdded('group', g.id) ? 'checked disabled' : '';
            const checkedAttr = isAdded('group', g.id) ? 'checked disabled' : '';
            return `
                <label class="checkbox-item modal-item-row" data-name="${g.name.toLowerCase()}">
                    <input type="checkbox" value="${g.id}" data-type="group" ${checkedAttr}>
                    <span>${g.name} ${isAdded('group', g.id) ? '<small style="color:var(--text-muted);">(added)</small>' : ''}</span>
                </label>
            `;
        }).join('');
    }

    // Populate Lights Checklist
    if (lightsList.length === 0) {
        lightsContainer.innerHTML = '<span style="color:var(--text-muted); font-size: 0.85rem; padding: 4px;">No lights found</span>';
    } else {
        lightsContainer.innerHTML = lightsList.map(l => {
            const checkedAttr = isAdded('light', l.id) ? 'checked disabled' : '';
            return `
                <label class="checkbox-item modal-item-row" data-name="${l.name.toLowerCase()}">
                    <input type="checkbox" value="${l.id}" data-type="light" ${checkedAttr}>
                    <span>${l.name} ${isAdded('light', l.id) ? '<small style="color:var(--text-muted);">(added)</small>' : ''}</span>
                </label>
            `;
        }).join('');
    }
}

function filterModalItems() {
    const query = document.getElementById('modal-search-input').value.toLowerCase().trim();
    const items = document.querySelectorAll('.modal-item-row');
    
    items.forEach(item => {
        const name = item.getAttribute('data-name');
        if (name.includes(query)) {
            item.style.display = 'flex';
        } else {
            item.style.display = 'none';
        }
    });
}

function submitAddWidgets() {
    const checkboxes = document.querySelectorAll('.modal-checklist input[type="checkbox"]:checked:not(:disabled)');
    
    if (checkboxes.length === 0) {
        closeAddWidgetsModal();
        return;
    }
    
    checkboxes.forEach(cb => {
        const type = cb.getAttribute('data-type');
        const id = cb.value;
        dashboardLayout.push({ type, id });
    });
    
    window.pywebview.api.save_dashboard_layout(dashboardLayout).then(() => {
        closeAddWidgetsModal();
        renderDashboardWidgets();
    });
}

function removeWidget(index) {
    if (confirm("Remove this widget control from dashboard layout?")) {
        dashboardLayout.splice(index, 1);
        window.pywebview.api.save_dashboard_layout(dashboardLayout).then(() => {
            renderDashboardWidgets();
        });
    }
}

/* User Controls Triggers */
function onDeviceToggle(type, id, checked) {
    const card = document.querySelector(`.device-card [id="${type}-${id}-toggle"]`);
    if (card) {
        const cardParent = card.closest('.device-card');
        if (cardParent) cardParent.classList.toggle('on', checked);
    }
    
    if (type === 'light') {
        window.pywebview.api.set_light_state(id, 'on', checked);
    } else {
        window.pywebview.api.set_group_state(id, 'on', checked);
    }
}

function onDeviceBrightnessInput(type, id, value) {
    const bri = parseInt(value);
    if (type === 'light') {
        window.pywebview.api.set_light_state(id, 'bri', bri);
    } else {
        window.pywebview.api.set_group_state(id, 'bri', bri);
    }
}

function onDeviceColorInput(type, id, hexValue) {
    const preview = document.getElementById(`${type}-${id}-color-preview`);
    if (preview) {
        preview.style.backgroundColor = hexValue;
    }
    
    const { hue, sat } = hexToHueSat(hexValue);
    
    if (type === 'light') {
        window.pywebview.api.set_light_state(id, 'hue', hue);
        window.pywebview.api.set_light_state(id, 'sat', sat);
    } else {
        window.pywebview.api.set_group_state(id, 'hue', hue);
        window.pywebview.api.set_group_state(id, 'sat', sat);
    }
}

/* MIDI & Mappings UI Logic */
function populateMidiConfig() {
    // 1. Populate MIDI Devices Dropdown
    window.pywebview.api.get_midi_devices().then(devices => {
        const select = document.getElementById('midi-device-select');
        select.innerHTML = '<option value="">-- Select MIDI Controller --</option>';
        
        devices.forEach(dev => {
            select.innerHTML += `<option value="${dev}">${dev}</option>`;
        });

        // Load currently selected MIDI device from config
        window.pywebview.api.get_selected_midi_device().then(selected => {
            if (selected) {
                select.value = selected;
                // Render mappings for this device
                renderMappings();
            }
        });
    });
}

function changeMidiDevice() {
    const select = document.getElementById('midi-device-select');
    const deviceName = select.value;
    
    window.pywebview.api.select_midi_device(deviceName).then(() => {
        renderMappings();
        refreshMidiStatus();
    });
}

function refreshMidiStatus() {
    if (!window.pywebview || !window.pywebview.api) return;
    
    window.pywebview.api.get_midi_status().then(info => {
        const badge = document.getElementById('midi-status-badge');
        
        if (info.status === 'listening') {
            badge.innerText = "Live Input: Active";
            badge.className = "learn-badge"; // normal green
            badge.title = "Device captured. Move a control to bind.";
        } else if (info.status === 'connecting') {
            badge.innerText = "Connecting...";
            badge.className = "learn-badge error"; // amber style or customized
            badge.title = "Opening MIDI device port...";
        } else if (info.status === 'error') {
            badge.innerText = "Conflict: Device Busy";
            badge.className = "learn-badge error"; // red style
            badge.title = `Error: ${info.error || "Device occupied by another application."}`;
        } else {
            badge.innerText = "Disconnected";
            badge.className = "learn-badge error"; // red/gray style
            badge.title = "No MIDI device active.";
        }
    });
}

function updateMidiTerminal(eventKey, value, learnCache) {
    const log = document.getElementById('midi-activity-log');
    
    if (learnCache.length === 0) {
        log.innerHTML = '<div class="terminal-line placeholder">Awaiting MIDI input...</div>';
        return;
    }

    log.innerHTML = learnCache.map(evt => {
        return `
            <div class="terminal-line" style="display: flex; justify-content: space-between; align-items: center; padding: 4px 0;">
                <span>⚡ Event: ${evt.key} &nbsp; [Val: ${evt.value}]</span>
                <button class="btn btn-secondary btn-sm" onclick="openMappingCreator('${evt.key}')">Bind</button>
            </div>
        `;
    }).join('');
}

/* Mapping Bind Form Management */
function openMappingCreator(midiKey, editingMapping = null) {
    const title = document.getElementById('mapping-creator-title');
    const saveBtn = document.getElementById('btn-save-mapping');
    
    document.getElementById('creator-midi-key').innerText = midiKey;
    document.getElementById('mapping-creator-card').classList.remove('hidden');
    
    if (editingMapping) {
        title.innerHTML = `Edit Mapping for <span id="creator-midi-key" class="neon-text">${midiKey}</span>`;
        saveBtn.innerText = "Update Mapping Bind";
        
        // Populate form with current values
        document.getElementById('mapping-target-type').value = editingMapping.target_type;
        populateTargetDropdown(); // populate ID list first
        document.getElementById('mapping-target-id').value = editingMapping.target_id;
        document.getElementById('mapping-action').value = editingMapping.action;
    } else {
        title.innerHTML = `Create Mapping for <span id="creator-midi-key" class="neon-text">${midiKey}</span>`;
        saveBtn.innerText = "Save Mapping Bind";
        
        // Pre-select defaults
        document.getElementById('mapping-target-type').value = 'light';
        populateTargetDropdown();
        document.getElementById('mapping-action').value = 'Toggle On/Off (Latch)';
    }
}

function closeMappingCreator() {
    document.getElementById('mapping-creator-card').classList.add('hidden');
}

function populateTargetDropdown() {
    const targetType = document.getElementById('mapping-target-type').value;
    const selectTargetId = document.getElementById('mapping-target-id');
    
    selectTargetId.innerHTML = '';
    
    if (targetType === 'light') {
        lightsList.forEach(light => {
            selectTargetId.innerHTML += `<option value="${light.id}">${light.name} (Light ${light.id})</option>`;
        });
    } else {
        groupsList.forEach(group => {
            selectTargetId.innerHTML += `<option value="${group.id}">${group.name} (Group ${group.id})</option>`;
        });
    }
}

function submitMapping() {
    const midiKey = document.getElementById('creator-midi-key').innerText;
    const targetType = document.getElementById('mapping-target-type').value;
    const targetId = document.getElementById('mapping-target-id').value;
    const action = document.getElementById('mapping-action').value;
    
    if (!targetId) {
        alert("Please select a target light or group.");
        return;
    }

    window.pywebview.api.add_mapping(midiKey, targetType, targetId, action).then(() => {
        closeMappingCreator();
        renderMappings();
    });
}

function renderMappings() {
    window.pywebview.api.get_mappings().then(mappings => {
        activeMappings = mappings || {};
        const tbody = document.getElementById('mappings-table-body');
        
        const keys = Object.keys(activeMappings);
        if (keys.length === 0) {
            tbody.innerHTML = `
                <tr>
                    <td colspan="5" class="empty-table">No mappings registered yet. Bind an event to start!</td>
                </tr>
            `;
            return;
        }

        tbody.innerHTML = keys.map(key => {
            const m = activeMappings[key];
            
            // Find names of target
            let targetName = `ID ${m.target_id}`;
            if (m.target_type === 'light') {
                const lObj = lightsList.find(l => l.id == m.target_id);
                if (lObj) targetName = lObj.name;
            } else {
                const gObj = groupsList.find(g => g.id == m.target_id);
                if (gObj) targetName = gObj.name;
            }

            return `
                <tr>
                    <td class="neon-text font-bold">${key}</td>
                    <td style="text-transform: capitalize;">${m.target_type}</td>
                    <td>${targetName}</td>
                    <td>${m.action}</td>
                    <td class="text-right">
                        <button class="btn-edit" onclick="editMapping('${key}')">Edit</button>
                        <button class="btn-delete" onclick="deleteMapping('${key}')">Delete</button>
                    </td>
                </tr>
            `;
        }).join('');
    });
}

function editMapping(midiKey) {
    const mapping = activeMappings[midiKey];
    if (mapping) {
        openMappingCreator(midiKey, mapping);
    }
}

function deleteMapping(midiKey) {
    if (confirm(`Remove mapping for ${midiKey}?`)) {
        window.pywebview.api.remove_mapping(midiKey).then(() => {
            renderMappings();
        });
    }
}

/* Color Space Conversions (Hex <-> Hue HSL) */
function hexToHueSat(hex) {
    hex = hex.replace(/^#/, '');
    
    let r = parseInt(hex.substring(0, 2), 16) / 255;
    let g = parseInt(hex.substring(2, 4), 16) / 255;
    let b = parseInt(hex.substring(4, 6), 16) / 255;
    
    let max = Math.max(r, g, b), min = Math.min(r, g, b);
    let h, s, l = (max + min) / 2;

    if (max === min) {
        h = s = 0; // achromatic
    } else {
        let d = max - min;
        s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
        switch (max) {
            case r: h = (g - b) / d + (g < b ? 6 : 0); break;
            case g: h = (b - r) / d + 2; break;
            case b: h = (r - g) / d + 4; break;
        }
        h /= 6;
    }
    
    return {
        hue: Math.round(h * 65535),
        sat: Math.round(s * 254)
    };
}

function hueSatToHex(hue, sat) {
    let h = hue / 65535;
    let s = sat / 254;
    let l = 0.5; // Medium lightness for color preview stability

    let r, g, b;
    if (s === 0) {
        r = g = b = l; // achromatic
    } else {
        const hue2rgb = (p, q, t) => {
            if (t < 0) t += 1;
            if (t > 1) t -= 1;
            if (t < 1/6) return p + (q - p) * 6 * t;
            if (t < 1/2) return q;
            if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
            return p;
        };
        let q = l < 0.5 ? l * (1 + s) : l + s - l * s;
        let p = 2 * l - q;
        r = hue2rgb(p, q, h + 1/3);
        g = hue2rgb(p, q, h);
        b = hue2rgb(p, q, h - 1/3);
    }
    
    const toHex = x => {
        const hex = Math.round(x * 255).toString(16);
        return hex.length === 1 ? '0' + hex : hex;
    };
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}
