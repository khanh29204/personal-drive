(function () {
  'use strict';

  var config = window.__DRIVE_CONFIG__ || {};
  var currentFolderId = config.currentFolderId;

  // ── Toast ──────────────────────────────────────────────────────
  function showToast(message, type) {
    var toast = document.getElementById('toast');
    if (!toast) return;
    toast.textContent = message;
    toast.className = 'toast ' + (type === 'error' ? 'toast-error' : 'toast-success');
    toast.style.display = 'block';
    setTimeout(function () {
      toast.style.display = 'none';
    }, 4000);
  }

  // ── Helper: API call with error handling ───────────────────────
  async function apiCall(url, options) {
    var res = await fetch(url, Object.assign({ credentials: 'include' }, options));
    if (!res.ok) {
      var data = {};
      try {
        data = await res.json();
      } catch (_e) {
        /* empty */
      }
      throw new Error(data.message || 'Lỗi ' + res.status);
    }
    if (res.status === 204) return null;
    return res.json();
  }

  // ── Login ──────────────────────────────────────────────────────
  var loginForm = document.getElementById('login-form');
  if (loginForm) {
    loginForm.addEventListener('submit', async function (event) {
      event.preventDefault();
      var formData = new FormData(loginForm);
      var errorEl = document.getElementById('login-error');
      errorEl.textContent = '';

      try {
        await apiCall('/api/auth/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            userName: formData.get('userName'),
            password: formData.get('password'),
          }),
        });
        window.location.reload();
      } catch (err) {
        errorEl.textContent = err.message || 'Đăng nhập thất bại';
      }
    });
  }

  // ── Logout ─────────────────────────────────────────────────────
  var logoutBtn = document.getElementById('logout-btn');
  if (logoutBtn) {
    logoutBtn.addEventListener('click', async function () {
      await fetch('/api/auth/logout', { method: 'POST', credentials: 'include' });
      window.location.reload();
    });
  }

  // ── Category Filter Bar Logic ────────────────────────────────────
  var filterPills = document.querySelectorAll('.filter-pill');
  if (filterPills.length > 0) {
    filterPills.forEach(function (pill) {
      pill.addEventListener('click', function () {
        filterPills.forEach(function (p) { p.classList.remove('active'); });
        pill.classList.add('active');

        var selectedCat = pill.getAttribute('data-category');
        var rows = document.querySelectorAll('.file-item-row');

        rows.forEach(function (row) {
          var rowCat = row.getAttribute('data-category');
          if (selectedCat === 'all' || rowCat === selectedCat) {
            row.style.display = '';
          } else {
            row.style.display = 'none';
          }
        });

        // Show parent row if present
        var parentRow = document.querySelector('.parent-row');
        if (parentRow) parentRow.style.display = '';
      });
    });
  }

  // ── Create Folder ──────────────────────────────────────────────
  var btnNewFolder = document.getElementById('btn-new-folder');
  if (btnNewFolder) {
    btnNewFolder.addEventListener('click', async function () {
      var name = prompt('Tên thư mục mới:');
      if (!name || !name.trim()) return;

      var isPublic = confirm('Đặt thư mục ở chế độ công khai?');

      try {
        await apiCall('/api/folders', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            name: name.trim(),
            parentId: currentFolderId || null,
            isPublic: isPublic,
          }),
        });
        showToast('Đã tạo thư mục "' + name.trim() + '"', 'success');
        window.location.reload();
      } catch (err) {
        showToast(err.message, 'error');
      }
    });
  }

  // ── Upload Files ───────────────────────────────────────────────
  var btnUpload = document.getElementById('btn-upload');
  var fileInput = document.getElementById('file-input');
  if (btnUpload && fileInput) {
    btnUpload.addEventListener('click', function () {
      fileInput.click();
    });

    fileInput.addEventListener('change', async function () {
      var files = fileInput.files;
      if (!files || files.length === 0) return;

      var progressArea = document.getElementById('upload-progress-area');
      var allDone = 0;
      var totalFiles = files.length;
      var hasError = false;

      for (var i = 0; i < files.length; i++) {
        (function (file, index) {
          // Create progress bar for this file
          var progressEl = document.createElement('div');
          progressEl.className = 'upload-progress';
          progressEl.innerHTML =
            '<span class="upload-filename">' +
            escapeHtml(file.name) +
            '</span>' +
            '<div class="upload-bar"><div class="upload-bar-fill" id="bar-' +
            index +
            '"></div></div>' +
            '<span class="upload-percent" id="percent-' +
            index +
            '">0%</span>';
          progressArea.appendChild(progressEl);

          // Step 1: Get upload URL
          apiCall('/api/files/upload-url', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              name: file.name,
              mimeType: file.type || 'application/octet-stream',
              size: file.size,
              folderId: currentFolderId || null,
              isPublic: false,
            }),
          })
            .then(function (data) {
              // Step 2: Upload to R2 via XHR for progress
              return new Promise(function (resolve, reject) {
                var xhr = new XMLHttpRequest();
                xhr.open('PUT', data.uploadUrl, true);
                xhr.setRequestHeader('Content-Type', file.type || 'application/octet-stream');

                xhr.upload.addEventListener('progress', function (e) {
                  if (e.lengthComputable) {
                    var pct = Math.round((e.loaded / e.total) * 100);
                    var barFill = document.getElementById('bar-' + index);
                    var percentEl = document.getElementById('percent-' + index);
                    if (barFill) barFill.style.width = pct + '%';
                    if (percentEl) percentEl.textContent = pct + '%';
                  }
                });

                xhr.onload = function () {
                  if (xhr.status >= 200 && xhr.status < 300) {
                    resolve(data.fileId);
                  } else {
                    reject(new Error('Upload lên R2 thất bại (status ' + xhr.status + ')'));
                  }
                };
                xhr.onerror = function () {
                  reject(new Error('Lỗi mạng khi upload'));
                };
                xhr.send(file);
              });
            })
            .then(function (fileId) {
              // Step 3: Complete upload
              return apiCall('/api/files/' + fileId + '/complete', {
                method: 'POST',
              });
            })
            .then(function () {
              var barFill = document.getElementById('bar-' + index);
              if (barFill) {
                barFill.style.width = '100%';
                barFill.style.background = '#22c55e';
              }
              allDone++;
              if (allDone === totalFiles) {
                showToast('Tải lên hoàn tất!', 'success');
                setTimeout(function () {
                  window.location.reload();
                }, 1000);
              }
            })
            .catch(function (err) {
              hasError = true;
              var barFill = document.getElementById('bar-' + index);
              if (barFill) barFill.style.background = '#ef4444';
              showToast(file.name + ': ' + err.message, 'error');
              allDone++;
              if (allDone === totalFiles && !hasError) {
                setTimeout(function () {
                  window.location.reload();
                }, 1000);
              }
            });
        })(files[i], i);
      }

      // Reset file input so the same files can be selected again
      fileInput.value = '';
    });
  }

  // ── Toggle Public/Private ──────────────────────────────────────
  window.togglePublic = async function (id, isDirectory, currentPublic) {
    var endpoint = isDirectory ? '/api/folders/' : '/api/files/';
    try {
      await apiCall(endpoint + id, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ isPublic: !currentPublic }),
      });
      showToast(currentPublic ? 'Đã chuyển sang riêng tư' : 'Đã chuyển sang công khai', 'success');
      window.location.reload();
    } catch (err) {
      showToast(err.message, 'error');
    }
  };

  // ── Delegated Event Listeners for Table Item Actions ────────────
  document.addEventListener('click', function (e) {
    var btnToggle = e.target.closest('.btn-toggle-public');
    if (btnToggle) {
      var id = btnToggle.getAttribute('data-id');
      var isDir = btnToggle.getAttribute('data-is-directory') === 'true';
      var isPub = btnToggle.getAttribute('data-is-public') === 'true';
      window.togglePublic(id, isDir, isPub);
      return;
    }

    var btnMove = e.target.closest('.btn-open-move');
    if (btnMove) {
      var idMove = btnMove.getAttribute('data-id');
      var isDirMove = btnMove.getAttribute('data-is-directory') === 'true';
      var nameMove = btnMove.getAttribute('data-name');
      window.openMoveModal(idMove, isDirMove, nameMove);
      return;
    }

    var btnEditLink = e.target.closest('.btn-open-edit-link');
    if (btnEditLink) {
      var idEdit = btnEditLink.getAttribute('data-id');
      var nameEdit = btnEditLink.getAttribute('data-name');
      var urlEdit = btnEditLink.getAttribute('data-url');
      var mimeEdit = btnEditLink.getAttribute('data-mime');
      window.openEditLinkModal(idEdit, nameEdit, urlEdit, mimeEdit);
      return;
    }

    var btnDelete = e.target.closest('.btn-delete-item');
    if (btnDelete) {
      var idDel = btnDelete.getAttribute('data-id');
      var typeDel = btnDelete.getAttribute('data-type');
      var nameDel = btnDelete.getAttribute('data-name');
      window.deleteItem(idDel, typeDel, nameDel);
      return;
    }
  });

  // ── Delete Item ────────────────────────────────────────────────
  window.deleteItem = async function (id, type, name) {
    if (!confirm('Xoá "' + name + '"? Không thể hoàn tác.')) return;
    var endpoint = type === 'folder' ? '/api/folders/' : '/api/files/';
    try {
      await apiCall(endpoint + id, { method: 'DELETE' });
      showToast('Đã xoá "' + name + '"', 'success');
      window.location.reload();
    } catch (err) {
      showToast(err.message, 'error');
    }
  };

  // ── Move Item ──────────────────────────────────────────────────
  var moveModal = document.getElementById('move-modal');
  var moveItemName = document.getElementById('move-item-name');
  var moveFolderSelect = document.getElementById('move-folder-select');
  var btnCancelMove = document.getElementById('btn-cancel-move');
  var btnConfirmMove = document.getElementById('btn-confirm-move');
  
  var currentMoveTarget = null;

  function populateMoveFolders() {
    if (!moveFolderSelect) return;
    moveFolderSelect.innerHTML = '<option value="">-- Màn hình chính --</option>';
    
    var folders = config.allFolders || [];
    if (Array.isArray(folders)) {
      folders.forEach(function(folder) {
        var folderId = (folder._id && typeof folder._id === 'object' && folder._id.$oid) 
          ? folder._id.$oid 
          : (folder._id || folder.id);

        if (folderId) {
          var option = document.createElement('option');
          option.value = folderId;
          option.textContent = folder.name;
          moveFolderSelect.appendChild(option);
        }
      });
    }
  }

  if (moveModal) {
    populateMoveFolders();

    if (btnCancelMove) {
      btnCancelMove.addEventListener('click', function() {
        moveModal.style.display = 'none';
        currentMoveTarget = null;
      });
    }

    if (btnConfirmMove) {
      btnConfirmMove.addEventListener('click', async function() {
        if (!currentMoveTarget) return;
        var endpoint = currentMoveTarget.isDirectory ? '/api/folders/' : '/api/files/';
        var body = currentMoveTarget.isDirectory 
          ? { parentId: moveFolderSelect.value || null } 
          : { folderId: moveFolderSelect.value || null };
        
        try {
          btnConfirmMove.disabled = true;
          await apiCall(endpoint + currentMoveTarget.id, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(body)
          });
          showToast('Đã di chuyển thành công', 'success');
          window.location.reload();
        } catch (err) {
          showToast(err.message, 'error');
          btnConfirmMove.disabled = false;
        }
      });
    }
  }

  window.openMoveModal = function(id, isDirectory, name) {
    if (!moveModal) return;
    currentMoveTarget = { id: id, isDirectory: isDirectory };
    moveItemName.textContent = name;
    populateMoveFolders();
    moveFolderSelect.value = ''; 
    
    var options = moveFolderSelect.options;
    for (var i = 0; i < options.length; i++) {
      if (isDirectory && options[i].value === id) {
        options[i].disabled = true;
      } else {
        options[i].disabled = false;
      }
    }
    
    moveModal.style.display = 'flex';
  };

  // ── Link File ──────────────────────────────────────────────────
  var btnLinkFile = document.getElementById('btn-link-file');
  var linkFileModal = document.getElementById('link-file-modal');
  var linkFileForm = document.getElementById('link-file-form');
  var btnCancelLink = document.getElementById('btn-cancel-link');

  if (btnLinkFile && linkFileModal && linkFileForm) {
    btnLinkFile.addEventListener('click', function() {
      linkFileForm.reset();
      linkFileModal.style.display = 'flex';
    });

    btnCancelLink.addEventListener('click', function() {
      linkFileModal.style.display = 'none';
    });

    linkFileForm.addEventListener('submit', async function(e) {
      e.preventDefault();
      var btnSubmit = linkFileForm.querySelector('button[type="submit"]');
      
      var name = document.getElementById('link-name').value.trim();
      var url = document.getElementById('link-url').value.trim();
      var mimeType = document.getElementById('link-mime').value.trim();

      if (!name || !url || !mimeType) return;

      try {
        btnSubmit.disabled = true;
        await apiCall('/api/files/link', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            name: name,
            url: url,
            mimeType: mimeType,
            folderId: currentFolderId || null
          })
        });
        showToast('Đã thêm liên kết file thành công', 'success');
        window.location.reload();
      } catch (err) {
        showToast(err.message, 'error');
        btnSubmit.disabled = false;
      }
    });
  }

  // ── Edit Link ──────────────────────────────────────────────────
  var editLinkModal = document.getElementById('edit-link-modal');
  var editLinkForm = document.getElementById('edit-link-form');
  var btnCancelEditLink = document.getElementById('btn-cancel-edit-link');

  window.openEditLinkModal = function(id, name, url, mime) {
    if (!editLinkModal) return;
    document.getElementById('edit-link-id').value = id;
    document.getElementById('edit-link-name').value = name;
    document.getElementById('edit-link-url').value = url;
    document.getElementById('edit-link-mime').value = mime;
    editLinkModal.style.display = 'flex';
  };

  if (editLinkModal && editLinkForm) {
    btnCancelEditLink.addEventListener('click', function() {
      editLinkModal.style.display = 'none';
    });

    editLinkForm.addEventListener('submit', async function(e) {
      e.preventDefault();
      var btnSubmit = editLinkForm.querySelector('button[type="submit"]');
      
      var id = document.getElementById('edit-link-id').value;
      var name = document.getElementById('edit-link-name').value.trim();
      var url = document.getElementById('edit-link-url').value.trim();
      var mimeType = document.getElementById('edit-link-mime').value.trim();

      if (!name || !url || !mimeType) return;

      try {
        btnSubmit.disabled = true;
        await apiCall('/api/files/' + id, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            name: name,
            url: url,
            mimeType: mimeType
          })
        });
        showToast('Đã cập nhật liên kết thành công', 'success');
        window.location.reload();
      } catch (err) {
        showToast(err.message, 'error');
        btnSubmit.disabled = false;
      }
    });
  }

  // ── Storage Quota & Interactive Orphan Management ──────────────────────────────
  async function loadStorageQuota() {
    var quotaText = document.getElementById('quota-text');
    var progressBar = document.getElementById('quota-progress-bar');
    if (!quotaText || !progressBar) return;

    try {
      var data = await apiCall('/api/files/storage/quota', { method: 'GET' });
      quotaText.textContent = data.usedFormatted + ' / ' + data.freeTierLimitFormatted + ' (' + data.usedPercentage + '%)';
      progressBar.style.width = Math.min(data.usedPercentage, 100) + '%';
      if (data.usedPercentage > 85) {
        progressBar.style.background = '#ef4444';
      } else if (data.usedPercentage > 70) {
        progressBar.style.background = '#f59e0b';
      } else {
        progressBar.style.background = 'linear-gradient(90deg, #2563eb, #3b82f6)';
      }
    } catch (_err) {
      quotaText.textContent = 'Lỗi nạp dung lượng';
    }
  }

  loadStorageQuota();

  var orphansModal = document.getElementById('orphans-modal');
  var btnCleanOrphans = document.getElementById('btn-clean-orphans');
  var btnCloseOrphansModal = document.getElementById('btn-close-orphans-modal');
  var orphansListContainer = document.getElementById('orphans-list-container');
  var btnDeleteSelectedOrphans = document.getElementById('btn-delete-selected-orphans');
  var btnCleanAllOrphansModal = document.getElementById('btn-clean-all-orphans-modal');
  var selectedOrphanCountEl = document.getElementById('selected-orphan-count');

  var currentOrphans = [];

  if (btnCleanOrphans && orphansModal) {
    btnCleanOrphans.addEventListener('click', function () {
      orphansModal.style.display = 'flex';
      loadOrphanFiles();
    });

    if (btnCloseOrphansModal) {
      btnCloseOrphansModal.addEventListener('click', function () {
        orphansModal.style.display = 'none';
      });
    }

    async function loadOrphanFiles() {
      if (!orphansListContainer) return;
      orphansListContainer.innerHTML = '<div style="text-align: center; padding: 24px; color: #6b7280;"><i class="fas fa-spinner fa-spin"></i> Đang quét dữ liệu R2...</div>';
      btnDeleteSelectedOrphans.style.display = 'none';

      try {
        currentOrphans = await apiCall('/api/files/storage/orphans', { method: 'GET' });
        renderOrphansList();
      } catch (err) {
        orphansListContainer.innerHTML = '<div style="text-align: center; padding: 24px; color: #dc2626;">' + escapeHtml(err.message) + '</div>';
      }
    }

    function renderOrphansList() {
      if (!currentOrphans || currentOrphans.length === 0) {
        orphansListContainer.innerHTML = '<div style="text-align: center; padding: 24px; color: #22c55e;"><i class="fas fa-check-circle"></i> Sạch sẽ! Không có file mồ côi nào trên Cloudflare R2.</div>';
        btnCleanAllOrphansModal.style.display = 'none';
        btnDeleteSelectedOrphans.style.display = 'none';
        return;
      }

      btnCleanAllOrphansModal.style.display = 'inline-flex';

      var html = '<table style="width:100%; border-collapse:collapse; font-size:13px;">' +
        '<thead><tr style="background:#f9fafb; border-bottom:1px solid #e5e7eb;">' +
        '<th style="width:36px; text-align:center; padding:8px;"><input type="checkbox" id="chk-select-all-orphans" /></th>' +
        '<th style="padding:8px; text-align:left;">Tên File trên R2</th>' +
        '<th style="padding:8px; text-align:right; width:100px;">Kích thước</th>' +
        '<th style="padding:8px; text-align:center; width:70px;">Hành động</th>' +
        '</tr></thead><tbody>';

      currentOrphans.forEach(function (item, idx) {
        html += '<tr style="border-bottom:1px solid #f3f4f6;">' +
          '<td style="text-align:center; padding:8px;"><input type="checkbox" class="chk-orphan-item" data-key="' + escapeHtml(item.key) + '" /></td>' +
          '<td style="padding:8px; word-break:break-all;"><i class="fas fa-file-alt text-muted" style="margin-right:6px;"></i>' + escapeHtml(item.name) + '</td>' +
          '<td style="padding:8px; text-align:right; color:#4b5563;">' + escapeHtml(item.sizeFormatted) + '</td>' +
          '<td style="padding:8px; text-align:center;"><button class="btn-icon btn-danger btn-delete-single-orphan" data-key="' + escapeHtml(item.key) + '" title="Xoá file này"><i class="fas fa-trash"></i></button></td>' +
          '</tr>';
      });

      html += '</tbody></table>';
      orphansListContainer.innerHTML = html;

      // Bind events
      var chkAll = document.getElementById('chk-select-all-orphans');
      var itemChks = document.querySelectorAll('.chk-orphan-item');

      if (chkAll) {
        chkAll.addEventListener('change', function () {
          itemChks.forEach(function (chk) { chk.checked = chkAll.checked; });
          updateSelectedOrphansCount();
        });
      }

      itemChks.forEach(function (chk) {
        chk.addEventListener('change', updateSelectedOrphansCount);
      });

      document.querySelectorAll('.btn-delete-single-orphan').forEach(function (btn) {
        btn.addEventListener('click', function () {
          var key = btn.getAttribute('data-key');
          deleteOrphanKeys([key]);
        });
      });
    }

    function updateSelectedOrphansCount() {
      var selected = document.querySelectorAll('.chk-orphan-item:checked');
      var count = selected.length;
      selectedOrphanCountEl.textContent = count;
      if (count > 0) {
        btnDeleteSelectedOrphans.style.display = 'inline-flex';
      } else {
        btnDeleteSelectedOrphans.style.display = 'none';
      }
    }

    async function deleteOrphanKeys(keys) {
      if (!keys || keys.length === 0) return;
      if (!confirm('Xoá ' + keys.length + ' file mồ côi đã chọn khỏi Cloudflare R2?')) return;

      try {
        var res = await apiCall('/api/files/storage/orphans', {
          method: 'DELETE',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ keys: keys }),
        });

        showToast('Đã xoá ' + res.deletedCount + ' file (' + res.freedFormatted + ')', 'success');
        await loadOrphanFiles();
        await loadStorageQuota();
      } catch (err) {
        showToast(err.message || 'Lỗi xoá file', 'error');
      }
    }

    if (btnDeleteSelectedOrphans) {
      btnDeleteSelectedOrphans.addEventListener('click', function () {
        var selected = document.querySelectorAll('.chk-orphan-item:checked');
        var keys = [];
        selected.forEach(function (chk) {
          keys.push(chk.getAttribute('data-key'));
        });
        deleteOrphanKeys(keys);
      });
    }

    if (btnCleanAllOrphansModal) {
      btnCleanAllOrphansModal.addEventListener('click', async function () {
        if (!confirm('Xoá tất cả file mồ côi đang có trên R2?')) return;
        try {
          btnCleanAllOrphansModal.disabled = true;
          var res = await apiCall('/api/files/storage/clean-orphans', { method: 'POST' });
          showToast('Đã xoá ' + res.deletedOrphanR2Objects + ' file mồ côi (' + res.freedFormatted + ')', 'success');
          await loadOrphanFiles();
          await loadStorageQuota();
        } catch (err) {
          showToast(err.message, 'error');
        } finally {
          btnCleanAllOrphansModal.disabled = false;
        }
      });
    }
  }

  // ── Global Modal Backdrop Click Handler ──────────────────────────
  window.addEventListener('click', function (e) {
    if (e.target.classList && e.target.classList.contains('modal')) {
      e.target.style.display = 'none';
    }
  });

  // ── Escape HTML helper ─────────────────────────────────────────
  function escapeHtml(str) {
    var div = document.createElement('div');
    div.appendChild(document.createTextNode(str));
    return div.innerHTML;
  }
})();



