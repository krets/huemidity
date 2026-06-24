// Global State
let connectionPollInterval = null;
let deviceUpdateInterval = null;
let midiStatusInterval = null;
let currentTab = 'lights';

let lightsList = [];
let groupsList = [];
let scenesList = [];
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

    // 3. Register Mapping action change listener
    const actionSel = document.getElementById('mapping-action');
    if (actionSel) {
        actionSel.addEventListener('change', updateMappingAutoOnVisibility);
    }

    // 4. Close custom dropdown when clicking outside
    window.addEventListener('click', (e) => {
        const select = document.getElementById('target-id-select-wrapper');
        const container = document.getElementById('custom-select-options');
        if (select && !select.contains(e.target)) {
            select.classList.remove('open');
            if (container) container.classList.add('hidden');
        }
    });
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
        if (window.pywebview.api.get_connection_error) {
            window.pywebview.api.get_connection_error().then(err => {
                document.getElementById('error-message-text').innerText = err || "Unable to connect to the Hue Bridge.";
            });
        }
    } else if (status === 'idle') {
        document.getElementById('state-manual-ip').classList.remove('hidden');
    }
}

function showManualIpForm() {
    if (connectionPollInterval) clearInterval(connectionPollInterval);
    updateOnboardingUI('idle');
}

async function submitManualIp() {
    const ip = document.getElementById('manual-ip-input').value.trim();
    if (!ip) {
        await showCustomAlert("Please enter a valid IP address.", "Invalid IP");
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

async function disconnectBridge() {
    const approved = await showCustomConfirm("Disconnect and forget the current Hue Bridge?", "Forget Hue Bridge");
    if (approved) {
        closeSettingsModal();
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
    
    const floatingBtn = document.getElementById('floating-add-btn');
    if (floatingBtn) {
        floatingBtn.classList.toggle('hidden', tabName !== 'lights');
    }
    
    if (tabName === 'midi') {
        refreshMidiStatus();
        renderMappings();
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
    // 1. Fetch layout and devices
    window.pywebview.api.get_dashboard_layout().then(layout => {
        dashboardLayout = layout || [];
        refreshDevices();
    });

    // 2. Populate MIDI configuration
    populateMidiConfig();
}

function refreshDevices() {
    if (!window.pywebview || !window.pywebview.api) return;
    
    window.pywebview.api.get_lights_and_groups().then(data => {
        lightsList = data.lights || [];
        groupsList = data.groups || [];
        scenesList = data.scenes || [];
        renderDashboardWidgets();
        renderMappings();
    });
}

function refreshDevicesSilent() {
    if (!window.pywebview || !window.pywebview.api) return;
    
    window.pywebview.api.get_lights_and_groups().then(data => {
        lightsList = data.lights || [];
        groupsList = data.groups || [];
        scenesList = data.scenes || [];
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
        let device = null;
        if (item.type === 'light') {
            device = lightsList.find(l => l.id == item.id);
        } else {
            device = groupsList.find(g => g.id == item.id);
        }
        
        if (!device) {
            device = { id: item.id, name: `Missing ${item.type} (${item.id})`, on: false, bri: 0, hue: 0, sat: 0, missing: true };
        }
        
        const cardHTML = createDeviceCard(device, item.type, index);
        container.insertAdjacentHTML('beforeend', cardHTML);
    });
    
    setupDragAndDropEvents();
    setupDashboardCardInteractions();
}

function setupDashboardCardInteractions() {
    const cards = document.querySelectorAll('.dashboard-widgets-grid .device-card');
    cards.forEach(card => {
        const index = parseInt(card.getAttribute('data-index'));
        const item = dashboardLayout[index];
        if (!item) return;

        // 1. Wheel/Scroll listener for brightness adjustment
        card.addEventListener('wheel', (e) => {
            e.preventDefault();
            
            let device = null;
            if (item.type === 'light') {
                device = lightsList.find(l => l.id == item.id);
            } else {
                device = groupsList.find(g => g.id == item.id);
            }
            if (!device || device.missing) return;

            const briSlider = document.getElementById(`${item.type}-${device.id}-bri`);
            if (!briSlider) return;

            let currentBri = parseInt(briSlider.value);
            const step = 15;
            if (e.deltaY < 0) {
                currentBri = Math.min(254, currentBri + step);
            } else {
                currentBri = Math.max(0, currentBri - step);
            }

            briSlider.value = currentBri;
            
            // Auto toggle ON if turned off and scrolling up
            const toggle = document.getElementById(`${item.type}-${device.id}-toggle`);
            if (toggle && !toggle.checked && e.deltaY < 0) {
                toggle.checked = true;
                onDeviceToggle(item.type, device.id, true);
            }

            onDeviceBrightnessInput(item.type, device.id, currentBri);
        }, { passive: false });

        // 2. Double-click listener to toggle power state
        card.addEventListener('dblclick', (e) => {
            if (e.target.tagName === 'INPUT' || e.target.tagName === 'BUTTON' || e.target.classList.contains('drag-handle') || e.target.closest('.color-picker-control')) {
                return;
            }
            
            let device = null;
            if (item.type === 'light') {
                device = lightsList.find(l => l.id == item.id);
            } else {
                device = groupsList.find(g => g.id == item.id);
            }
            if (!device || device.missing) return;

            const toggle = document.getElementById(`${item.type}-${device.id}-toggle`);
            if (toggle) {
                const newChecked = !toggle.checked;
                toggle.checked = newChecked;
                onDeviceToggle(item.type, device.id, newChecked);
            }
        });
    });
}

function createDeviceCard(device, type, index) {
    const isOn = device.on;
    const hexColor = hueSatToHex(device.hue, device.sat);
    const isMissing = device.missing;
    const hasDim = device.capabilities && device.capabilities.includes('dim');
    const hasColor = device.capabilities && device.capabilities.includes('color');
    
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
            
            ${!isMissing && (hasDim || hasColor) ? `
            <div class="device-controls-row">
                ${hasDim ? `
                <div class="slider-control">
                    <input type="range" id="${type}-${device.id}-bri" min="0" max="254" value="${device.bri}" 
                           oninput="onDeviceBrightnessInput('${type}', '${device.id}', this.value)">
                </div>
                ` : ''}
                ${hasColor ? `
                <div class="color-picker-control">
                    <div class="color-picker-wrapper" style="background-color: ${hexColor}" id="${type}-${device.id}-color-preview">
                        <input type="color" id="${type}-${device.id}-color" value="${hexColor}" 
                               oninput="onDeviceColorInput('${type}', '${device.id}', this.value)">
                    </div>
                </div>
                ` : ''}
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
    const cards = document.querySelectorAll('.dashboard-widgets-grid .device-card');
    
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
        scenesList = data.scenes || [];
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

async function removeWidget(index) {
    const approved = await showCustomConfirm("Remove this widget control from dashboard layout?", "Remove Widget");
    if (approved) {
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
            badge.className = "learn-badge";
            badge.title = "Device captured. Move a control to bind.";
        } else if (info.status === 'connecting') {
            badge.innerText = "Connecting...";
            badge.className = "learn-badge error";
            badge.title = "Opening MIDI device port...";
        } else if (info.status === 'error') {
            badge.innerText = "Conflict: Device Busy";
            badge.className = "learn-badge error";
            badge.title = `Error: ${info.error || "Device occupied by another application."}`;
        } else {
            badge.innerText = "Disconnected";
            badge.className = "learn-badge error";
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
        const timestamp = evt.time || "YYYY-MM-DD HH:mm:ss";
        return `
            <div class="terminal-line">
                <span class="timestamp">${timestamp}</span>
                <span class="event-key">${evt.key}</span>
                <span class="divider">:</span>
                <span class="event-value">${evt.value}</span>
                <span class="event-space-fill"></span>
                <button class="btn btn-secondary btn-sm btn-bind" onclick="openMappingCreator('${evt.key}')">Bind</button>
            </div>
        `;
    }).join('');
}

/* Centered Mapping Creator Modal Management */
function openMappingCreator(midiKey, editingMapping = null) {
    const title = document.getElementById('mapping-creator-title');
    const saveBtn = document.getElementById('btn-save-mapping');
    
    document.getElementById('creator-midi-key').innerText = midiKey;
    document.getElementById('mapping-creator-modal').classList.remove('hidden');
    
    if (editingMapping) {
        title.innerHTML = `Edit Mapping for <span id="creator-midi-key" class="neon-text">${midiKey}</span>`;
        saveBtn.innerText = "Update Mapping Bind";
        
        document.getElementById('mapping-target-type').value = editingMapping.target_type;
        populateTargetDropdown();
        document.getElementById('mapping-target-id').value = editingMapping.target_id;
        
        // Populate actions list based on capabilities first, then set value
        onMappingTargetChanged();
        document.getElementById('mapping-action').value = editingMapping.action;
        
        // Load checkbox states
        document.getElementById('mapping-invert').checked = editingMapping.invert || false;
        document.getElementById('mapping-auto-on').checked = editingMapping.auto_on || false;
        
        // Update custom dropdown selected text
        updateCustomSelectTriggerLabel();
    } else {
        title.innerHTML = `Create Mapping for <span id="creator-midi-key" class="neon-text">${midiKey}</span>`;
        saveBtn.innerText = "Save Mapping Bind";
        
        document.getElementById('mapping-target-type').value = 'light';
        populateTargetDropdown();
        
        // Select first target option by default if available
        const selectTargetId = document.getElementById('mapping-target-id');
        if (selectTargetId.options.length > 0) {
            selectTargetId.selectedIndex = 0;
        }
        
        onMappingTargetChanged();
        updateCustomSelectTriggerLabel();
        
        // Reset checkboxes
        document.getElementById('mapping-invert').checked = false;
        document.getElementById('mapping-auto-on').checked = false;
    }
    updateMappingAutoOnVisibility();
}

function onMappingTargetTypeChanged() {
    populateTargetDropdown();
    const selectTargetId = document.getElementById('mapping-target-id');
    if (selectTargetId.options.length > 0) {
        selectTargetId.selectedIndex = 0;
    } else {
        selectTargetId.value = '';
    }
    onMappingTargetChanged();
    updateCustomSelectTriggerLabel();
}

function closeMappingCreator() {
    document.getElementById('mapping-creator-modal').classList.add('hidden');
}

function populateTargetDropdown() {
    const targetType = document.getElementById('mapping-target-type').value;
    const selectTargetId = document.getElementById('mapping-target-id');
    const customContainer = document.getElementById('custom-select-options');
    
    selectTargetId.innerHTML = '';
    customContainer.innerHTML = '';
    
    if (targetType === 'light') {
        lightsList.forEach(light => {
            const deviceType = light.type || 'Light';
            const optionText = `${light.name} (${light.id}: ${deviceType})`;
            
            selectTargetId.innerHTML += `<option value="${light.id}">${optionText}</option>`;
            
            customContainer.innerHTML += `
                <div class="custom-option-item" onclick="selectCustomOption('${light.id}')">
                    <span class="option-title">${light.name}</span>
                    <span class="option-meta">(${light.id}: ${deviceType})</span>
                </div>
            `;
        });
    } else if (targetType === 'group') {
        groupsList.forEach(group => {
            const deviceType = group.type || 'Group';
            const optionText = `${group.name} (${group.id}: ${deviceType})`;
            
            selectTargetId.innerHTML += `<option value="${group.id}">${optionText}</option>`;
            
            customContainer.innerHTML += `
                <div class="custom-option-item" onclick="selectCustomOption('${group.id}')">
                    <span class="option-title">${group.name}</span>
                    <span class="option-meta">(${group.id}: ${deviceType})</span>
                </div>
            `;
        });
    } else if (targetType === 'scene') {
        scenesList.forEach(scene => {
            const groupObj = groupsList.find(g => g.id == scene.group_id);
            const groupName = groupObj ? groupObj.name : `Group ${scene.group_id}`;
            const optionText = `${scene.name} (${scene.id}: Scene in ${groupName})`;
            const value = `${scene.group_id}/${scene.id}`;
            
            selectTargetId.innerHTML += `<option value="${value}">${optionText}</option>`;
            
            customContainer.innerHTML += `
                <div class="custom-option-item" onclick="selectCustomOption('${value}')">
                    <span class="option-title">${scene.name}</span>
                    <span class="option-meta">(${scene.id}: Scene in ${groupName})</span>
                </div>
            `;
        });
    }
}

/* Filter Available Actions based on target hardware capabilities */
function onMappingTargetChanged() {
    const targetType = document.getElementById('mapping-target-type').value;
    const targetId = document.getElementById('mapping-target-id').value;
    const actionSelect = document.getElementById('mapping-action');
    
    actionSelect.innerHTML = '';
    
    if (targetType === 'scene') {
        actionSelect.innerHTML = '<option value="Recall Scene">Recall Scene</option>';
        updateMappingAutoOnVisibility();
        return;
    }
    
    // Core actions (all lights & groups support this)
    let actions = [
        { val: 'Toggle On/Off (Latch)', label: 'Toggle On/Off (Latch)' },
        { val: 'Toggle On/Off (Momentary)', label: 'Toggle On/Off (Momentary)' }
    ];
    
    if (targetType === 'group') {
        actions.push({ val: 'Brightness', label: 'Brightness' });
        actions.push({ val: 'Color Temperature', label: 'Color Temperature (CT)' });
        actions.push({ val: 'Hue', label: 'Hue' });
        actions.push({ val: 'Saturation', label: 'Saturation' });
        actions.push({ val: 'Red', label: 'Red Component' });
        actions.push({ val: 'Green', label: 'Green Component' });
        actions.push({ val: 'Blue', label: 'Blue Component' });
    } else if (targetType === 'light') {
        const light = lightsList.find(l => l.id == targetId);
        if (light && light.capabilities) {
            if (light.capabilities.includes('dim')) {
                actions.push({ val: 'Brightness', label: 'Brightness' });
            }
            if (light.capabilities.includes('ct')) {
                actions.push({ val: 'Color Temperature', label: 'Color Temperature (CT)' });
            }
            if (light.capabilities.includes('color')) {
                actions.push({ val: 'Hue', label: 'Hue' });
                actions.push({ val: 'Saturation', label: 'Saturation' });
                actions.push({ val: 'Red', label: 'Red Component' });
                actions.push({ val: 'Green', label: 'Green Component' });
                actions.push({ val: 'Blue', label: 'Blue Component' });
            }
        }
    }
    
    actions.forEach(act => {
        actionSelect.innerHTML += `<option value="${act.val}">${act.label}</option>`;
    });
    updateMappingAutoOnVisibility();
}

async function submitMapping() {
    const midiKey = document.getElementById('creator-midi-key').innerText;
    const targetType = document.getElementById('mapping-target-type').value;
    const targetId = document.getElementById('mapping-target-id').value;
    const action = document.getElementById('mapping-action').value;
    const invert = document.getElementById('mapping-invert').checked;
    const autoOn = document.getElementById('mapping-auto-on').checked;
    
    if (!targetId) {
        await showCustomAlert("Please select a target device or scene.", "Select Target");
        return;
    }

    window.pywebview.api.add_mapping(midiKey, targetType, targetId, action, invert, autoOn).then(() => {
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
            
            // 1. Icon type mapping
            let typeIcon = '💡';
            if (m.target_type === 'group') typeIcon = '📦';
            if (m.target_type === 'scene') typeIcon = '🎬';
            
            // 2. Target name mapping
            let targetName = `ID ${m.target_id}`;
            if (m.target_type === 'light') {
                const lObj = lightsList.find(l => l.id == m.target_id);
                if (lObj) targetName = lObj.name;
            } else if (m.target_type === 'group') {
                const gObj = groupsList.find(g => g.id == m.target_id);
                if (gObj) targetName = gObj.name;
            } else if (m.target_type === 'scene') {
                // Scene target_id contains group_id/scene_id
                if (m.target_id.includes('/')) {
                    const [gId, sId] = m.target_id.split('/', 2);
                    const scObj = scenesList.find(s => s.id == sId);
                    const grObj = groupsList.find(g => g.id == gId);
                    const gName = grObj ? grObj.name : `Group ${gId}`;
                    if (scObj) {
                        targetName = `${scObj.name} (${gName})`;
                    } else {
                        targetName = `Scene (${gName})`;
                    }
                }
            }

            // 3. Concise Action Label Mapping
            let actionLabel = m.action;
            if (m.action.startsWith('Toggle On/Off')) {
                actionLabel = "On/Off";
            } else if (m.action === "Color Temperature") {
                actionLabel = "Color Temp";
            }

            return `
                <tr>
                    <td class="neon-text font-bold">${key}</td>
                    <td style="text-align: center; font-size: 1.1rem;" title="${m.target_type}">${typeIcon}</td>
                    <td>${targetName}</td>
                    <td>${actionLabel}</td>
                    <td class="text-right">
                        <button class="btn-edit" onclick="editMapping('${key}')" title="Edit Bind">✏️</button>
                        <button class="btn-delete" onclick="deleteMapping('${key}')" title="Delete Bind">🗑️</button>
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

async function deleteMapping(midiKey) {
    const approved = await showCustomConfirm(`Remove mapping for ${midiKey}?`, "Remove Mapping");
    if (approved) {
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

/* Settings Modal Management */
function openSettingsModal() {
    window.pywebview.api.get_bridge_ip().then(ip => {
        document.getElementById('settings-bridge-ip').innerText = ip || "Unknown";
    });
    window.pywebview.api.get_config_path().then(path => {
        document.getElementById('settings-config-path').innerText = path || "Unknown";
    });
    window.pywebview.api.get_autostart().then(enabled => {
        document.getElementById('settings-autostart').checked = enabled;
    });
    document.getElementById('settings-modal').classList.remove('hidden');
}

function onAutostartChanged() {
    const enabled = document.getElementById('settings-autostart').checked;
    window.pywebview.api.set_autostart(enabled);
}

function closeSettingsModal() {
    document.getElementById('settings-modal').classList.add('hidden');
}

async function quitApp() {
    const approved = await showCustomConfirm("Completely exit and close HueMIDIty?", "Quit Application");
    if (approved) {
        closeSettingsModal();
        document.getElementById('shutdown-overlay').classList.remove('hidden');
        setTimeout(() => {
            window.pywebview.api.quit_application();
        }, 150);
    }
}

/* Auto-On visibility helper */
function updateMappingAutoOnVisibility() {
    const action = document.getElementById('mapping-action').value;
    const wrapper = document.getElementById('mapping-auto-on-wrapper');
    if (!wrapper) return;
    
    // Auto-On applies to value controls, NOT toggles or scene recall
    const isValueControl = action === 'Brightness' || action === 'Color Temperature' || 
                           action === 'Hue' || action === 'Saturation' || 
                           action === 'Red' || action === 'Green' || action === 'Blue';
    if (isValueControl) {
        wrapper.classList.remove('hidden');
    } else {
        wrapper.classList.add('hidden');
        document.getElementById('mapping-auto-on').checked = false;
    }
}

/* Custom Alert/Confirm Modal Promisified Dialogs */
let confirmResolve = null;

function showCustomAlert(message, title = "Notification") {
    document.getElementById('alert-title').innerText = title;
    document.getElementById('alert-message').innerText = message;
    document.getElementById('alert-btn-cancel').classList.add('hidden');
    document.getElementById('alert-btn-ok').innerText = "OK";
    
    document.getElementById('custom-alert-modal').classList.remove('hidden');
    
    return new Promise(resolve => {
        confirmResolve = () => {
            document.getElementById('custom-alert-modal').classList.add('hidden');
            resolve(true);
        };
    });
}

function showCustomConfirm(message, title = "Confirm Action") {
    document.getElementById('alert-title').innerText = title;
    document.getElementById('alert-message').innerText = message;
    document.getElementById('alert-btn-cancel').classList.remove('hidden');
    document.getElementById('alert-btn-ok').innerText = "Confirm";
    
    document.getElementById('custom-alert-modal').classList.remove('hidden');
    
    return new Promise(resolve => {
        confirmResolve = (val) => {
            document.getElementById('custom-alert-modal').classList.add('hidden');
            resolve(val);
        };
    });
}

function resolveCustomConfirm(value) {
    if (confirmResolve) {
        confirmResolve(value);
    }
}

/* Custom Dropdown Trigger Handlers */
function toggleCustomSelect(e) {
    if (e) e.stopPropagation();
    const wrapper = document.getElementById('target-id-select-wrapper');
    const container = document.getElementById('custom-select-options');
    if (!wrapper || !container) return;
    
    const isOpen = wrapper.classList.contains('open');
    if (isOpen) {
        wrapper.classList.remove('open');
        container.classList.add('hidden');
    } else {
        wrapper.classList.add('open');
        container.classList.remove('hidden');
    }
}

function selectCustomOption(value) {
    const nativeSelect = document.getElementById('mapping-target-id');
    if (nativeSelect) {
        nativeSelect.value = value;
        nativeSelect.dispatchEvent(new Event('change'));
    }
    
    updateCustomSelectTriggerLabel();
    
    const wrapper = document.getElementById('target-id-select-wrapper');
    const container = document.getElementById('custom-select-options');
    if (wrapper) wrapper.classList.remove('open');
    if (container) container.classList.add('hidden');
}

function updateCustomSelectTriggerLabel() {
    const targetType = document.getElementById('mapping-target-type').value;
    const targetId = document.getElementById('mapping-target-id').value;
    const triggerTextEl = document.getElementById('custom-select-selected-text');
    
    if (!targetId) {
        triggerTextEl.innerHTML = '<span class="option-title">Select Target Device</span>';
        return;
    }
    
    let html = '';
    if (targetType === 'light') {
        const light = lightsList.find(l => l.id == targetId);
        if (light) {
            html = `<span class="option-title">${light.name}</span> <span class="option-meta">(${light.id}: ${light.type || 'Light'})</span>`;
        } else {
            html = `<span class="option-title">Light ${targetId}</span> <span class="option-meta">(${targetId})</span>`;
        }
    } else if (targetType === 'group') {
        const group = groupsList.find(g => g.id == targetId);
        if (group) {
            html = `<span class="option-title">${group.name}</span> <span class="option-meta">(${group.id}: ${group.type || 'Group'})</span>`;
        } else {
            html = `<span class="option-title">Group ${targetId}</span> <span class="option-meta">(${targetId})</span>`;
        }
    } else if (targetType === 'scene') {
        if (targetId.includes('/')) {
            const [gId, sId] = targetId.split('/', 2);
            const scene = scenesList.find(s => s.id == sId);
            const group = groupsList.find(g => g.id == gId);
            const gName = group ? group.name : `Group ${gId}`;
            if (scene) {
                html = `<span class="option-title">${scene.name}</span> <span class="option-meta">(${scene.id}: Scene in ${gName})</span>`;
            } else {
                html = `<span class="option-title">Scene ${sId}</span> <span class="option-meta">(${targetId})</span>`;
            }
        } else {
            const scene = scenesList.find(s => s.id == targetId);
            if (scene) {
                const group = groupsList.find(g => g.id == scene.group_id);
                const gName = group ? group.name : `Group ${scene.group_id}`;
                html = `<span class="option-title">${scene.name}</span> <span class="option-meta">(${scene.id}: Scene in ${gName})</span>`;
            } else {
                html = `<span class="option-title">Scene ${targetId}</span> <span class="option-meta">(${targetId})</span>`;
            }
        }
    }
    triggerTextEl.innerHTML = html;
}
