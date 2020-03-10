function onClickPostImage(ev) {
    if (ev.target.dataset.expanded) {
        delete ev.target.dataset.expanded
        ev.target.src = ev.target.dataset.thumbUri
    } else {
        ev.target.dataset.expanded = "expanded"
        ev.target.src = ev.target.dataset.uri
    }
}

function onClickPostId(ev) {
    ev.preventDefault()

    var id = ev.target.textContent.replace("#", "")
    var reference = ">>" + id + "\n\n"

    var textarea = document.querySelector('.new-item-form textarea')

    textarea.value += reference
    textarea.focus()
}

function onMouseEnterPostRef(ev) {
    var targetRect = ev.target.getBoundingClientRect()

    var id = ev.target.textContent.replace(">>", "")

    var post = document.getElementById(id)

    if (post === null) {
        var url = window.location.href.replace(/#.*/gi, "")
        url += "/preview/" + id

        fetch(url)
            .then((response) => {
                return response.text()
            })
            .then((content) => {
                var template = document.createElement('template')
                template.innerHTML = content

                addPostPreview(template.content.firstChild, targetRect)
            })
    } else {
        addPostPreview(post.cloneNode(true), targetRect)
    }
}

function addPostPreview(postPreview, targetRect) {
    postPreview.classList.add("post-preview")

    document.body.appendChild(postPreview)

    var previewRect = postPreview.getBoundingClientRect()

    var maxPreviewTop = window.innerHeight - previewRect.height - 40

    var previewTop = targetRect.bottom - (previewRect.height / 2)
    previewTop = Math.max(previewTop, 0)
    previewTop = Math.min(previewTop, maxPreviewTop)

    var previewMaxWidth = window.innerWidth - targetRect.right - 40

    postPreview.style.top = previewTop + "px"
    postPreview.style.left = (targetRect.right + 10) + "px"
    postPreview.style.maxWidth = previewMaxWidth + "px"
}

function onMouseLeavePostRef(ev) {
    document.querySelectorAll('.post-preview').forEach((elem) => {
        elem.remove()
    })
}

document.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('.post-image img').forEach((elem) => {
        elem.addEventListener('click', onClickPostImage)
    })

    document.querySelectorAll('.post .post-id').forEach((elem) => {
        elem.addEventListener('click', onClickPostId)
    })

    document.querySelectorAll('.post-ref').forEach((elem) => {
        elem.addEventListener('mouseenter', onMouseEnterPostRef)
    })

    document.querySelectorAll('.post-ref').forEach((elem) => {
        elem.addEventListener('mouseleave', onMouseLeavePostRef)
    })
})
