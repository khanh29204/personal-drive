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

  if (moveModal && config.allFolders) {
    config.allFolders.forEach(function(folder) {
      var option = document.createElement('option');
      option.value = folder._id;
      option.textContent = folder.name;
      moveFolderSelect.appendChild(option);
    });

    btnCancelMove.addEventListener('click', function() {
      moveModal.style.display = 'none';
      currentMoveTarget = null;
    });

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

  window.openMoveModal = function(id, isDirectory, name) {
    if (!moveModal) return;
    currentMoveTarget = { id: id, isDirectory: isDirectory };
    moveItemName.textContent = name;
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

  // ── Escape HTML helper ─────────────────────────────────────────
  function escapeHtml(str) {
    var div = document.createElement('div');
    div.appendChild(document.createTextNode(str));
    return div.innerHTML;
  }
})();
